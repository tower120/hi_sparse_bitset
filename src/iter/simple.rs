use crate::bitset_interface::{BitSetBase, LevelMasks};
use crate::bit_queue::BitQueue;
use crate::{BitBlock, data_block_start_index, DataBlock, DataBlockIter};
use crate::config::Config;

/// Simple iterator - access each data block, by traversing all hierarchy
/// levels indirections each time.
///
/// Does not cache intermediate level1 position - hence have smaller size.
/// All Cache parameters will be ignored. Consider using [CachingBlockIter]
/// with [cache::NoCache] instead.
///
/// May have similar to [CachingBlockIter] performance on very sparse sets.
/// 
/// [cache::NoCache]: crate::cache::NoCache
pub struct SimpleBlockIter<T>
where
    T: LevelMasks,
{
    virtual_set: T,
    
    level0_iter: <<T::Conf as Config>::Level0BitBlock as BitBlock>::BitsIter,
    level1_iter: <<T::Conf as Config>::Level1BitBlock as BitBlock>::BitsIter,
    level0_index: usize,
}

impl<T> SimpleBlockIter<T>
where
    T: LevelMasks
{
    #[inline]
    pub fn new(virtual_set: T) -> Self {
        let level0_iter = virtual_set.level0_mask().into_bits_iter();
        Self{
            virtual_set,
            level0_iter,
            level1_iter: BitQueue::empty(),
            level0_index: 0
        }
    }
}


impl<T> Iterator for SimpleBlockIter<T>
where
    T: LevelMasks,
{
    type Item = DataBlock<<<T as BitSetBase>::Conf as Config>::DataBitBlock>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let level1_index = loop{
            if let Some(index) = self.level1_iter.next(){
                break index;
            } else {
                //update level0
                if let Some(index) = self.level0_iter.next(){
                    self.level0_index = index;

                    // update level1 iter
                    let level1_mask = unsafe {
                        self.virtual_set.level1_mask(index)
                    };
                    self.level1_iter = level1_mask.into_bits_iter();
                } else {
                    return None;
                }
            }
        };

        let data_mask = unsafe {
            self.virtual_set.data_mask(self.level0_index, level1_index)
        };

        let block_start_index =
            data_block_start_index::<<T as BitSetBase>::Conf>(
                self.level0_index, level1_index
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