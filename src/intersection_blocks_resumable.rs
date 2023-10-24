//! This allows to iterate intersection result in several iteration sessions.
//! Even when queried HiSparseBitSets mutated between sessions!
//!
//! It is guaranteed to iterate all non-removed intersected elements
//! that existed at the moment of suspension. It may return some newly added,
//! that satisfy intersection constraints as well.

use std::ops;
use num_traits::AsPrimitive;
use arrayvec::ArrayVec;
use crate::bit_block::BitBlock;
use crate::bit_queue::BitQueue;
use crate::block::Block;
use crate::level::Level;

use super::{bit_op, DataBlock, HiSparseBitset, IConfig, Level1Block};

pub struct IntersectionBlocksState<Config: IConfig> {
    level0_iter: <Config::Level0BitBlock as BitBlock>::BitsIter,
    level1_iter: <Config::Level1BitBlock as BitBlock>::BitsIter,
    level0_index: usize,
}
impl<Config: IConfig> Default for IntersectionBlocksState<Config>{
    /// It is safe to use any sets with default constructed `IntersectionBlocksState`.
    #[inline]
    fn default() -> Self {
        Self { 
            level0_iter: BitQueue::filled(),
            level1_iter: BitQueue::empty(),
            level0_index: 0
        }
    }
}
impl<Config: IConfig> IntersectionBlocksState<Config> {
    /// Every time you call `resume`, `sets` must point to the same [HiSparseBitset]s for each `state`.
    /// Otherwise - it is safe, but you'll get garbage out.
    /// 
    /// It is allowed for pointed [HiSparseBitset]s to change their state.
    /// On resume, it is guarantee that elements that all had to be iterated before calling [suspend] will
    /// be iterated, except those who was removed, or does not intersects any more.
    /// 
    /// See [update].
    #[inline]
    pub fn resume<'a, S>(self, sets: S) -> IntersectionBlocks<'a, Config, S>
    where
        S: Iterator<Item = &'a HiSparseBitset<Config>> + Clone,
        S: ExactSizeIterator
    {
        // Is it ok to have empty level1_blocks?
        let level1_blocks = IntersectionBlocks::<Config, S>::make_level1_blocks(sets.len());
        let mut this = IntersectionBlocks{sets, state: self, level1_blocks };
        this.update();
        this
    }
}

type Level1Blocks<Config: IConfig> = ArrayVec<*const Level1Block<Config>, 32>;

// TODO: undo this behavior.
/// May return empty blocks during iteration!
pub struct IntersectionBlocks<'a, Config, S>
where
    Config: IConfig,
    S: Iterator<Item = &'a HiSparseBitset<Config>> + Clone
{
    sets: S,
    state: IntersectionBlocksState<Config>,
    level1_blocks: Level1Blocks<Config>
}

impl<'a, Config, S> IntersectionBlocks<'a, Config, S>
where
    Config: IConfig,
    S: Iterator<Item = &'a HiSparseBitset<Config>> + Clone,
{
    fn make_level1_blocks(sets_len: usize) -> Level1Blocks<Config> {
        unsafe {
            let mut array = ArrayVec::new();
            array.set_len(sets_len);
            array
        }
    }

    #[inline]
    pub(super) fn new(sets: S) -> Self
    where
        S: ExactSizeIterator
    {
        // Level0
        let level0_iter = match Self::level0_intersection(sets.clone()){
            Some(intersection) => intersection.bits_iter(),
            None => BitQueue::empty(),
        };

        let sets_len = sets.len();

        Self {
            sets, 
            state: IntersectionBlocksState{
                level0_iter,
                level1_iter: BitQueue::empty(),
                level0_index: 0,
            },
            level1_blocks: Self::make_level1_blocks(sets_len)
        }
    }

    #[inline]
    pub fn suspend(self) -> IntersectionBlocksState<Config>{
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
        let Self{sets, state, level1_blocks} = self;

        // Level0
        let level0_intersection = match Self::level0_intersection(sets.clone()){
            Some(intersection) => intersection,
            None => {
                // empty sets - rare case.
                state.level0_iter = BitQueue::empty();
                return;
            },
        };
        let level0_index_valid = level0_intersection.get_bit(state.level0_index);
        state.level0_iter.mask_out(level0_intersection.as_array_u64());

        // Level1
        if level0_index_valid{
            let level1_intersection = Self::level1_intersection(sets.clone(), state.level0_index);
            state.level1_iter.mask_out(level1_intersection.as_array_u64());
        } else {
            // We already update level0_iter - we do not
            // update level0_index too, since it will be updated in iterator.
            state.level1_iter  = BitQueue::empty();
        }

        // TODO: put into if?
        Self::update_level1_blocks(level1_blocks, sets.clone(), state.level0_index);
    }

    #[inline]
    fn level0_intersection(sets: S) -> Option<Config::Level0BitBlock>{
        sets
        .map(|set| *set.level0.mask())
        .reduce(ops::BitAnd::bitand)
    }    

    #[inline]
    fn level1_intersection(sets: S, level0_index: usize) -> Config::Level1BitBlock{
        unsafe{
            sets
            .map(|set| {
                let level1_block_index = set.level0.get_unchecked(level0_index);
                let level1_block = set.level1.blocks().get_unchecked(level1_block_index.as_());
                *level1_block.mask()
            })
            .reduce(ops::BitAnd::bitand)
            .unwrap_unchecked()
        }
    }

    #[inline]
    fn update_level1_blocks(level1_blocks: &mut Level1Blocks<Config>, sets: S, level0_index: usize){
        // update level1_blocks from sets
        unsafe {
            for (index, set) in sets.enumerate(){
                let level1_block_index = set.level0.get_unchecked(level0_index);
                let level1_block       = set.level1.blocks().get_unchecked(level1_block_index.as_());

                *level1_blocks.get_unchecked_mut(index) = level1_block /*as *mut _*/ as * const _;
            }
        }
    }
}


impl<'a, Config, S> Iterator for IntersectionBlocks<'a, Config, S>
where
    Config: IConfig,
    S: Iterator<Item = &'a HiSparseBitset<Config>> + Clone,
{
    type Item = DataBlock<Config::DataBitBlock>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self{sets, state, level1_blocks } = self;

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
                    state.level1_iter = level1_intersection.bits_iter();

                    // update level1_blocks from sets
                    Self::update_level1_blocks(level1_blocks, sets.clone(), state.level0_index);
                } else {
                    return None;
                }
            }
        };
        
        let data_intersection = unsafe{
            let mut level1_blocks_iter = level1_blocks.iter();
            sets.clone()
            .map(|set| {
                let level1_block = &**level1_blocks_iter.next().unwrap_unchecked();
                let data_block_index   = level1_block.get_unchecked(level1_index);
                *set.data.blocks().get_unchecked(data_block_index.as_()).mask()
            })
            .reduce(ops::BitAnd::bitand)
            .unwrap_unchecked()
        };

        let block_start_index = (state.level0_index << (Config::DataBitBlock::SIZE_POT_EXPONENT + Config::Level1BitBlock::SIZE_POT_EXPONENT))
                              + (level1_index << (Config::DataBitBlock::SIZE_POT_EXPONENT));

        Some(DataBlock{ start_index: block_start_index, bit_block: data_intersection })
    }
}