use std::marker::PhantomData;
use num_traits::AsPrimitive;
use crate::binary_op::BinaryOp;
use crate::{data_block_start_index, DataBlock, IConfig};
use crate::bit_block::BitBlock;
use crate::bit_queue::BitQueue;
use crate::reduce2::State;
use crate::virtual_bitset::{LevelMasks, LevelMasksExt3};

/// Simple iterator - access each data block, by traversing all hierarchy
/// levels indirections each time.
///
/// Does not cache intermediate level1 position - hence have MUCH smaller size.
/// May have similar to [Iter] performance on very sparse sets.
///
/// # Motivation
///
/// The only reason why you might want to use this - is size.
/// `SimpleIter` according to benchmarks can be up to x2 slower,
/// but usually difference around x1.5.
pub struct SimpleIter<T>
where
    T: LevelMasks,
{
    virtual_set: T,
    state: State<T::Config>,
}

impl<T> SimpleIter<T>
where
    T: LevelMasks
{
    #[inline]
    pub fn new(virtual_set: T) -> Self {
        let state = State{
            level0_iter: virtual_set.level0_mask().bits_iter(),
            level1_iter: BitQueue::empty(),
            level0_index: 0,
        };
        Self::with_state(virtual_set, state)
    }

    #[inline]
    pub fn with_state(virtual_set: T, state: State<T::Config>) -> Self{
        Self{
            virtual_set,
            state,
        }
    }
}


impl<T> Iterator for SimpleIter<T>
where
    T: LevelMasks,
{
    type Item = DataBlock<<<T as LevelMasks>::Config as IConfig>::DataBitBlock>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self{ virtual_set, state, ..} = self;

        let level1_index =
            loop{
                if let Some(index) = state.level1_iter.next(){
                    break index;
                } else {
                    //update level0
                    if let Some(index) = state.level0_iter.next(){
                        state.level0_index = index;

                        // update level1 iter
                        let level1_intersection = unsafe {
                            virtual_set.level1_mask(index.as_())
                        };
                        state.level1_iter = level1_intersection.bits_iter();
                    } else {
                        return None;
                    }
                }
            };

        let data_intersection = unsafe {
            self.virtual_set.data_mask(state.level0_index, level1_index)
        };

        let block_start_index =
            data_block_start_index::<<T as LevelMasks>::Config>(
                state.level0_index, level1_index
            );

        Some(DataBlock{ start_index: block_start_index, bit_block: data_intersection })
    }
}


/// Fast on all operations.
///
/// Cache level1 block pointers, making data blocks access faster.
///
/// Also, can discard (on branch level) sets with empty level1 blocks from iteration.
/// (See [binary_op] - this have no effect for AND operation, but can speed up all other)
///
/// N.B. Do not move or clone without need - heavyweight due to cache.
pub struct IterExt3<T>
where
    T: LevelMasksExt3,
{
    virtual_set: T,
    state: State<T::Config>,
    level1_blocks: T::Level1Blocks3,
}

impl<T> IterExt3<T>
where
    T: LevelMasksExt3,
{
    #[inline]
    pub fn new(virtual_set: T) -> Self {
        let state = State{
            level0_iter: virtual_set.level0_mask().bits_iter(),
            level1_iter: BitQueue::empty(),
            level0_index: 0,
        };
        Self::with_state(virtual_set, state)
    }

    #[inline]
    pub fn with_state(virtual_set: T, state: State<T::Config>) -> Self{
        let level1_blocks = virtual_set.make_level1_blocks3();
        Self{
            virtual_set,
            state,
            level1_blocks
        }
    }
}


impl<T> Iterator for IterExt3<T>
where
    T: LevelMasksExt3,
{
    type Item = DataBlock<<T::Config as IConfig>::DataBitBlock>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self{virtual_set, state, level1_blocks, ..} = self;

        let level1_index =
            loop{
                if let Some(index) = state.level1_iter.next(){
                    break index;
                } else {
                    //update level0
                    if let Some(index) = state.level0_iter.next(){
                        state.level0_index = index;

                        // new style
                        {
                            let (level1_intersection, valid) = unsafe {
                                virtual_set.always_update_level1_blocks3(level1_blocks, state.level0_index)
                            };
                            if !valid {
                                // level1_mask can not be empty here
                                unsafe { std::hint::unreachable_unchecked() }
                            }
                            state.level1_iter = level1_intersection.bits_iter();
                        }

                        /*// old style
                        {
                            // update level1 iter
                            let level1_intersection = unsafe {
                                virtual_set.level1_mask(index.as_())
                            };
                            state.level1_iter = level1_intersection.bits_iter();

                            // update level1_blocks from sets
                            unsafe {
                                virtual_set.update_level1_blocks3(level1_blocks, state.level0_index);
                            }
                        }*/
                    } else {
                        return None;
                    }
                }
            };

        let data_intersection = unsafe {
            //self.reduce.
            T::data_mask_from_blocks3(level1_blocks, level1_index)
        };

        let block_start_index =
            data_block_start_index::<<T as LevelMasks>::Config>(
                state.level0_index, level1_index
            );

        Some(DataBlock{ start_index: block_start_index, bit_block: data_intersection })
    }
}
