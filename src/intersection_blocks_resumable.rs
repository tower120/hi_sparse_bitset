//! This allows to iterate intersection result in several iteration sessions.
//! Even when queried HiSparseBitSets mutated between sessions!
//!
//! It is guaranteed to iterate all non-removed intersected elements
//! that existed at the moment of suspension. It may return some newly added,
//! that satisfy intersection constraints as well.

use replace_with::{replace_with, replace_with_or_abort};

use crate::utils::{simd_op::{SimdOp, OneIndicesIterator}, bit_op::get_raw_array_bit, index_pool::State};

use super::{Level0Mask, Level1Mask, DataBlock, HiSparseBitset};

pub struct IntersectionBlocksState {
    level0_iter: <Level0Mask as SimdOp>::OneIndices,
    level1_iter: <Level1Mask as SimdOp>::OneIndices,
    level0_index: usize,
}
impl Default for IntersectionBlocksState{
    /// It is safe to use any sets with default constructed `IntersectionBlocksState`.
    #[inline]
    fn default() -> Self {
        Self { 
            level0_iter: <Level0Mask as SimdOp>::OneIndices::from_raw([i64::MAX; 2], 0), 
            level1_iter: <Level1Mask as SimdOp>::OneIndices::empty(),
            level0_index: 0
        }
    }
}
impl IntersectionBlocksState{
    /// Every time you call `resume`, `sets` must point to the same [HiSparseBitset]s for each `state`.
    /// Otherwise - it is safe, but you'll get garbage out.
    /// 
    /// It is allowed for pointed [HiSparseBitset]s to change their state.
    /// On resume, it is guarantee that elements that all had to be iterated before calling [suspend] will
    /// be iterated, except those who was removed, or does not intersects any more.
    /// 
    /// See [update].
    #[inline]
    pub fn resume<'a, S>(self, sets: S) -> IntersectionBlocks<'a, S>
    where
        S: Iterator<Item = &'a HiSparseBitset> + Clone
    {
        let mut this = IntersectionBlocks{sets, state: self};
        this.update();
        this
    }
}

/// May return empty blocks during iteration!
pub struct IntersectionBlocks<'a, S>
where
    S: Iterator<Item = &'a HiSparseBitset> + Clone
{
    sets: S,
    state: IntersectionBlocksState    
}

impl<'a, S> IntersectionBlocks<'a, S>
where
    S: Iterator<Item = &'a HiSparseBitset> + Clone
{
    #[inline]
    pub(super) fn new(sets: S) -> Self {
        // Level0
        let level0_iter = match Self::level0_intersection(sets.clone()){
            Some(intersection) => {
                SimdOp::one_indices(intersection)
            },
            None => <Level0Mask as SimdOp>::OneIndices::empty(),
        };

        Self { 
            sets, 
            state: IntersectionBlocksState{
                level0_iter,
                level1_iter: <Level1Mask as SimdOp>::OneIndices::empty(),
                level0_index: 0,
            }
        }
    }

    #[inline]
    pub fn suspend(self) -> IntersectionBlocksState{
        self.state
    }

    /// Patch/fix iterator.
    /// 
    /// Iteration will proceed from where it stopped last time.
    /// All removed elements will not appear in iteration.
    /// But newly appeared elements may appear in iteration.
    /// 
    /// # Safety 
    /// 
    /// `sets` must point to the same `HiSparseBitset`s as before, otherwise
    /// you'll get garbage in out.
    #[inline]
    fn update(&mut self){
        let Self{sets, state} = self;

        // Level0
        let level0_intersection = match Self::level0_intersection(sets.clone()){
            Some(intersection) => intersection,
            None => {
                // empty sets - rare case.
                state.level0_iter = <Level0Mask as SimdOp>::OneIndices::empty();
                return;
            },
        };
        let level0_intersection = level0_intersection.as_array_i64();
        let level0_index_valid = unsafe{ get_raw_array_bit(level0_intersection.as_ptr() as *const _, state.level0_index) };
        update_iter(&mut state.level0_iter, level0_intersection);

        // Level1
        if level0_index_valid{
            let level1_intersection = Self::level1_intersection(sets.clone(), state.level0_index);
            update_iter(&mut state.level1_iter, level1_intersection.as_array_i64());
        } else {
            // We already update level0_iter - we does not
            // update level0_index too, since it will be updated in iterator.
            state.level1_iter  = <Level1Mask as SimdOp>::OneIndices::empty();
        }

        #[inline]
        fn update_iter<Iter: OneIndicesIterator>(iter: &mut Iter, intersection: &[i64]){
            // OneIndicesIterator zeroing passed bits, so `iter` block will
            // act as mask for passed indices, and also will mask-out indices
            // that was not in original intersection. 

            replace_with(
                iter, 
                ||unsafe{
                    // We know that we don't panic here
                    std::hint::unreachable_unchecked()
                },
                |iter|{
                    let (mut blocks, block_index) = iter.into_raw();    
                    for i in 0..intersection.len(){   // compiletime unwinded loop
                        blocks[i] &= intersection[i];
                    }
                    Iter::from_raw(blocks, block_index)
                }
            );
        }
    }

    #[inline]
    fn level0_intersection(sets: S) -> Option<Level0Mask>{
        sets
        .map(|set| *set.level0.mask())
        .reduce(SimdOp::and)
    }    

    #[inline]
    fn level1_intersection(sets: S, level0_index: usize) -> Level1Mask{
        unsafe{
            sets
            .map(|set| {
                let level1_block_index = set.level0.get_unchecked(level0_index);
                let level1_block = set.level1.blocks().get_unchecked(level1_block_index as usize);
                *level1_block.mask()
            })
            .reduce(SimdOp::and)
            .unwrap_unchecked()
        }
    }    
}


impl<'a, S> Iterator for IntersectionBlocks<'a, S>
where
    S: Iterator<Item = &'a HiSparseBitset> + Clone
{
    type Item = (usize, DataBlock);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self{sets, state} = self;

        let level1_index =
        loop{
            if let Some(index) = state.level1_iter.next(){
                break index;
            } else {
                //update level0
                if let Some(index) = state.level0_iter.next(){
                    state.level0_index = index;

                    // update level1 iter
                    let level1_intersection = Self::level1_intersection(sets.clone(), index);
                    state.level1_iter = SimdOp::one_indices(level1_intersection);
                } else {
                    return None;
                }
            }
        };
        
        let data_intersection = unsafe{
            sets.clone()
            .map(|set| {
                // We could collect level1_block_index/&level1_block during level1 walk,
                // but benchmarks showed that does not have measurable performance benefits.
                // TODO: consider caching this in self
                let level1_block_index = set.level0.get_unchecked(state.level0_index);
                let level1_block       = set.level1.blocks().get_unchecked(level1_block_index as usize);

                let data_block_index   = level1_block.get_unchecked(level1_index);
                *set.data.blocks().get_unchecked(data_block_index as usize).mask()
            })
            .reduce(SimdOp::and)
            .unwrap_unchecked()
        };

        let block_start_index = (state.level0_index << (DataBlock::SIZE_POT_EXPONENT + Level1Mask::SIZE_POT_EXPONENT))
                              + (level1_index << (DataBlock::SIZE_POT_EXPONENT));

        Some((block_start_index, data_intersection))
    }
}