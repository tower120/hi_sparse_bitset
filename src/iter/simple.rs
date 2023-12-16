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
    type Conf = T::Conf;

    #[inline]
    fn cursor(&self) -> BlockCursor<Self::Conf> {
        unimplemented!()
    }

    type IndexIter = SimpleIndexIter<T>;

    #[inline]
    fn into_indices(self) -> Self::IndexIter {
        SimpleIndexIter::new(self)
    }

    fn move_to(self, _cursor: BlockCursor<Self::Conf>) -> Self {
        unimplemented!()
    }

    fn traverse<F>(self, _f: F) -> ControlFlow<()> 
    where 
        F: FnMut(DataBlock<<Self::Conf as Config>::DataBitBlock>) -> ControlFlow<()> 
    {
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

// It's just flatmap across block iterator.
pub struct SimpleIndexIter<T>
where
    T: LevelMasks
{
    block_iter: SimpleBlockIter<T>,
    data_block_iter: DataBlockIter<<T::Conf as Config>::DataBitBlock>,
}
impl<T> SimpleIndexIter<T>
where
    T: LevelMasks
{
    #[inline]
    pub fn new(block_iter: SimpleBlockIter<T>) -> Self{
        Self{
            block_iter,
            data_block_iter: DataBlockIter{
                start_index: 0,
                bit_block_iter: BitQueue::empty()
            }
        }
    }
}
impl<T> IndexIterator for SimpleIndexIter<T>
where
    T: LevelMasks
{
    type Conf = T::Conf;

    fn cursor(&self) -> IndexCursor<Self::Conf> {
        unimplemented!()
    }

    fn move_to(self, _cursor: IndexCursor<Self::Conf>) -> Self {
        unimplemented!()
    }

    fn traverse<F>(self, _f: F) -> ControlFlow<()> where F: FnMut(usize) -> ControlFlow<()> {
        unimplemented!()
    }
}
impl<T> Iterator for SimpleIndexIter<T>
where
    T: LevelMasks
{
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // TODO: ?? Still empty blocks ??
        // looping, because BlockIter may return empty DataBlocks.
        loop{
            if let Some(index) = self.data_block_iter.next(){
                return Some(index);
            }

            if let Some(data_block) = self.block_iter.next(){
                self.data_block_iter = data_block.into_iter();
            } else {
                return None;
            }
        }
    }
}