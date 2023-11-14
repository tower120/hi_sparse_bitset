use std::mem::{ManuallyDrop, MaybeUninit};
use crate::bit_block::BitBlock;
use crate::bit_queue::BitQueue;
use crate::bitset_interface::LevelMasksExt;
use super::*;

/// Caching iterator.
///
/// Cache pre-data level block pointers, making data blocks access faster.
/// Also, can discard (on pre-data level) sets with empty level1 blocks from iteration.
/// (See [binary_op] - this have no effect for AND operation, but can speed up all other)
///
/// # Real performance
///
/// ?? Inaccurate data ??
///
/// Thou, this iterator has algorithmically lower complexity, then [SimpleBlockIter],
/// due to the fact that modern processors are able to cache and access 1-indirection
/// hops as they're not a thing, simple iterator can outperform this more sophisticated
/// iterator machinery on small sets(both in set size and sets count), and very sparse sets
/// (where each data block occupy only one level1 block).
///
/// According to benchmarks difference looks not critical, and range from x2 slower
/// to x2 faster in extreme cases. Usually, difference is around 30-50%.
///
/// Since this is block iterator(you'll have to iterate it anyway), and it is an EXTREMELY fast
/// in any case, so fast that even tiny changes in benchmarks results in severely different numbers  -
/// it is hard to tell - which one iterator is faster in real-use.
///
/// # Memory footprint
///
/// Do not move or clone without need - heavyweight due to cache.
/// Memory footprint comes mainly from [reduce cache].
///
/// [reduce cache]: crate::cache
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
        let state = State{
            level0_iter: virtual_set.level0_mask().bits_iter(),
            level1_iter: BitQueue::empty(),
            level0_index: 0,
        };
        let cache_data = virtual_set.make_cache();
        Self{
            virtual_set,
            state,
            cache_data: ManuallyDrop::new(cache_data),
            level1_blocks: MaybeUninit::uninit()
        }
    }

    // TODO: rename, and consider making resume() from State.
    fn resume(virtual_set: T, mut state: State<T::Config>) -> Self {
        let mut cache_data = virtual_set.make_cache();
        let mut level1_blocks = MaybeUninit::uninit();
        let lvl1_mask_gen = |index| unsafe {
            // Generate both mask and level1_blocks cache
            let (mask, valid) = virtual_set.update_level1_blocks(
                &mut cache_data, &mut level1_blocks, index
            );
            if !valid {
                // level1_mask can not be empty here
                std::hint::unreachable_unchecked()
            }
            mask
        };
        patch_state(&mut state, &virtual_set, lvl1_mask_gen);

        Self{
            virtual_set,
            state,
            cache_data: ManuallyDrop::new(cache_data),
            level1_blocks
        }
    }

    #[inline]
    fn suspend(self) -> State<T::Config> {
        // drop in usual way, and return state.
        unsafe { std::ptr::read(&self.state) }
    }

    type IndexIter = CachingIndexIter<T>;

    #[inline]
    fn as_indices(self) -> CachingIndexIter<T>{
        CachingIndexIter::new(self)
    }
}


impl<T> Iterator for CachingBlockIter<T>
where
    T: LevelMasksExt,
{
    type Item = DataBlock<<T::Config as IConfig>::DataBitBlock>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self{virtual_set, state, cache_data, level1_blocks} = self;

        let level1_index =
            loop{
                if let Some(index) = state.level1_iter.next(){
                    break index;
                } else {
                    //update level0
                    if let Some(index) = state.level0_iter.next(){
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
            data_block_start_index::<<T as LevelMasks>::Config>(
                state.level0_index, level1_index
            );

        Some(DataBlock{ start_index: block_start_index, bit_block: data_intersection })
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