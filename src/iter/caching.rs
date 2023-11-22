use std::mem::{ManuallyDrop, MaybeUninit};
use crate::bit_block::BitBlock;
use crate::bit_queue::BitQueue;
use crate::bitset_interface::{BitSetBase, LevelMasksExt};
use super::*;

/// Caching iterator.
///
/// Cache pre-data level block pointers, making data blocks access faster.
/// Also, can discard (on pre-data level) sets with empty level1 blocks from iteration.
/// (See [binary_op] - this have no effect for AND operation, but can speed up all other)
///
/// # Memory footprint
///
/// This iterator may cache some data.
/// Amount of memory used by cache depends on [cache type].
/// Cache affects only [reduce] operations.
pub struct CachingBlockIter<T>
where
    T: LevelMasksExt,
{
    virtual_set: T,
    state: State<T::Config>,
    cache_data: ManuallyDrop<T::CacheData>,
    /// Never drop - since we're guaranteed to have them POD.
    level1_blocks: MaybeUninit<T::Level1Blocks>,
}

impl<T> BlockIterator for CachingBlockIter<T>
where
    T: LevelMasksExt,
{
    type BitSet = T;

    #[inline]
    fn new(virtual_set: T) -> Self {
        let level0_iter = virtual_set.level0_mask().bits_iter();
        //let level0_index = level0_iter.current();
        let state = State{
            level0_iter,
            level1_iter: BitQueue::empty(),
            level0_index: 0,    // Could be Uninit as well
        };
        let cache_data = virtual_set.make_cache();
        Self{
            virtual_set,
            state,
            cache_data: ManuallyDrop::new(cache_data),
            level1_blocks: MaybeUninit::uninit()
        }
    }

    #[inline]
    fn cursor(&self) -> BlockIterCursor {
        BlockIterCursor {
            level0_index: self.state.level0_index,
            level1_index: self.state.level1_iter.current(),
        }
    }

    type IndexIter = CachingIndexIter<T>;

    #[inline]
    fn as_indices(self) -> CachingIndexIter<T> {
        CachingIndexIter::new(self)
    }

    #[inline]
    fn skip_to(&mut self, cursor: BlockIterCursor) {
        // use actual level0 index.
        let level0_index = self.state.level0_iter.current();

        use std::cmp::Ordering;
        match Ord::cmp(&cursor.level0_index, &level0_index){
            Ordering::Less => {
                // we're ahead of cursor
                return;
            }
            Ordering::Equal => {
                // We never inited level1_iter before?
                if self.state.level1_iter.is_empty(){
                    // actually consume level0 bit
                    {                        
                        let index = self.state.level0_iter.next();
                        if index.is_none(){
                            // bitset completely empty - nothing to do
                            return;
                        }
                        debug_assert_eq!(index.unwrap(), level0_index);
                    }

                    let (level1_mask, valid) = unsafe {
                        self.virtual_set.update_level1_blocks(&mut self.cache_data, &mut self.level1_blocks, level0_index)
                    };
                    if !valid {
                        // level1_mask can not be empty here
                        unsafe { std::hint::unreachable_unchecked() }
                    }

                    self.state.level1_iter = level1_mask.bits_iter();
                }

                // mask out level1
                /*unsafe {
                    self.state.level1_iter.zero_first_n_unchecked(cursor.level1_index);
                }*/
                self.state.level1_iter.zero_first_n(cursor.level1_index);
            }
            Ordering::Greater => {
                // we're before cursor
                /*unsafe {
                    self.state.level0_iter.zero_first_n_unchecked(cursor.level0_index);
                }*/
                self.state.level0_iter.zero_first_n(cursor.level0_index);
                self.state.level0_index = cursor.level0_index;
                self.state.level1_iter  = BitQueue::empty();
            }
        }
    }    
}

impl<T> Iterator for CachingBlockIter<T>
where
    T: LevelMasksExt,
{
    type Item = DataBlock<<T::Config as IConfig>::DataBitBlock>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self { virtual_set, state, cache_data, level1_blocks } = self;

        let level1_index =
            loop {
                if let Some(index) = state.level1_iter.next() {
                    break index;
                } else {
                    //update level0
                    if let Some(index) = state.level0_iter.next() {
                        state.level0_index = index;

                        let (level1_mask, valid) = unsafe {
                            virtual_set.update_level1_blocks(cache_data, level1_blocks, index)
                        };
                        if !valid {
                            // level1_mask can not be empty here
                            unsafe { std::hint::unreachable_unchecked() }
                        }
                        state.level1_iter = level1_mask.bits_iter();
                    } else {
                        return None;
                    }
                }
            };

        let data_intersection = unsafe {
            T::data_mask_from_blocks(level1_blocks.assume_init_ref(), level1_index)
        };

        let block_start_index =
            data_block_start_index::<<T as BitSetBase>::Config>(
                state.level0_index, level1_index,
            );

        Some(DataBlock { start_index: block_start_index, bit_block: data_intersection })
    }
}

impl<T> Drop for CachingBlockIter<T>
    where
        T: LevelMasksExt
{
    #[inline]
    fn drop(&mut self) {
        self.virtual_set.drop_cache(&mut self.cache_data);
    }
}

pub type CachingIndexIter<T> = IndexIter<CachingBlockIter<T>>;