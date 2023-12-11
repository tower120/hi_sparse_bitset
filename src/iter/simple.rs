use crate::bitset_interface::{BitSetBase, LevelMasksExt};
use crate::bitset_interface::LevelMasks;
use crate::bit_queue::BitQueue;
use num_traits::AsPrimitive;
use crate::data_block_start_index;
use super::*;

/// Simple iterator - access each data block, by traversing all hierarchy
/// levels indirections each time.
///
/// Does not cache intermediate level1 position - hence have smaller size.
/// All Cache parameters will be ignored. Consider using [CachingIterator]
/// with [cache::NoCache] instead.
///
/// May have similar to [CachingIterator] performance on very sparse sets.
pub struct SimpleBlockIter<T>
where
    T: LevelMasks,
{
    virtual_set: T,
    state: State<T::Conf>,
}

impl<T> SimpleBlockIter<T>
where
    T: LevelMasksExt
{
    #[inline]
    pub fn new(virtual_set: T) -> Self {
        let state = State{
            level0_iter: virtual_set.level0_mask().bits_iter(),
            level1_iter: BitQueue::empty(),
            level0_index: 0,
        };
        Self{
            virtual_set,
            state,
        }
    }
}


impl<T> BlockIterator for SimpleBlockIter<T>
where
    T: LevelMasksExt
{
    type DataBitBlock = <T::Conf as Config>::DataBitBlock;

    #[inline]
    fn cursor(&self) -> BlockCursor {
        unimplemented!()
        /*BlockIterCursor{
            level0_index: self.state.level0_index,
            level1_index: self.state.level1_iter.current(),
        }*/
    }

    type IndexIter = SimpleIndexIter<T>;

    #[inline]
    fn as_indices(self) -> Self::IndexIter {
        SimpleIndexIter::new(self)
    }

    fn move_to(self, _cursor: BlockCursor) -> Self {
        unimplemented!()
    }
}


impl<T> Iterator for SimpleBlockIter<T>
where
    T: LevelMasks,
{
    type Item = DataBlock<<<T as BitSetBase>::Conf as Config>::DataBitBlock>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self{ virtual_set, state} = self;

        let level1_index =
            loop{
                if let Some(index) = state.level1_iter.next(){
                    break index;
                } else {
                    //update level0
                    if let Some(index) = state.level0_iter.next(){
                        state.level0_index = index;

                        // update level1 iter
                        let level1_mask = unsafe {
                            virtual_set.level1_mask(index.as_())
                        };
                        state.level1_iter = level1_mask.bits_iter();
                    } else {
                        return None;
                    }
                }
            };

        let data_mask = unsafe {
            self.virtual_set.data_mask(state.level0_index, level1_index)
        };

        let block_start_index =
            data_block_start_index::<<T as BitSetBase>::Conf>(
                state.level0_index, level1_index
            );

        Some(DataBlock{ start_index: block_start_index, bit_block: data_mask })
    }
}

pub type SimpleIndexIter<T> = IndexIter<SimpleBlockIter<T>>;