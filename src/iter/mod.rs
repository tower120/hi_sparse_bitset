//! Iteration always return ordered (or sorted) index sequences.

use crate::{DataBlock, DataBlockIter};
use crate::bit_block::BitBlock;
use crate::config::Config;

mod caching;
pub use caching::{CachingBlockIter, CachingIndexIter};

#[cfg(feature = "simple_iter")]
mod simple;
#[cfg(feature = "simple_iter")]
pub use simple::{SimpleBlockIter, SimpleIndexIter};

#[inline]
fn data_block_start_index<Conf: Config>(level0_index: usize, level1_index: usize) -> usize{
    let level0_offset = level0_index << (Conf::DataBitBlock::SIZE_POT_EXPONENT + Conf::Level1BitBlock::SIZE_POT_EXPONENT);
    let level1_offset = level1_index << (Conf::DataBitBlock::SIZE_POT_EXPONENT);
    level0_offset + level1_offset
}

// TODO: Consider making Copy
/// Block iterator cursor, or position of iterable.
/// 
/// Created by [BlockIterator::cursor()], used by [BlockIterator::move_to()].
/// 
/// Allows to resume iteration from the last position, even if the
/// source was mutated. Can be used with any [BitSetInterface].
/// Default constructed cursor will traverse bitset from the very begin.
/// 
/// # Use-case
///
/// This can be used to split long iteration into a few sessions.
/// You may want that in concurrent environment, when you can't process whole
/// iteration sequence fast, and want not to keep lock
/// on resource all the time you process iteration sequence.
/// 
/// Example: you lock sets, make intersection iterator, read 40 blocks into
/// buffer, suspend iterator to state, unlock sets, process buffer, lock sets,
/// resume iterator from state, and so on.
/// 
/// [BitSetInterface]: crate::BitSetInterface
#[derive(Default, Clone)]
pub struct BlockIterCursor{
    // TODO: u32's ?
    pub(crate) level0_index: usize,
    // We don't have current/last returned index
    pub(crate) level1_next_index: usize,
}

/// Index iterator cursor.
/// 
/// Created by [IndexIterator::cursor()], used by [IndexIterator::move_to()].
/// 
/// Same as [BlockIterCursor], but for indices iterator.
#[derive(Default, Clone)]
pub struct IndexIterCursor{
    pub(crate) block_cursor: BlockIterCursor,
    // TODO: u32's ?
    pub(crate) data_next_index: usize,
}

// TODO: move inside caching iterator
pub(crate) struct State<Conf: Config> {
    pub(crate) level0_iter: <Conf::Level0BitBlock as BitBlock>::BitsIter,
    pub(crate) level1_iter: <Conf::Level1BitBlock as BitBlock>::BitsIter,
    pub(crate) level0_index: usize,
}

/// Block iterator.
/// 
/// # Empty blocks
/// 
/// Block iterator may occasionally return empty blocks.
/// This is for performance reasons - since you most likely will
/// traverse block indices in loop anyway - checking it for emptiness, and then looping to the 
/// next non-empty one inside BlockIterator - may be just unnecessary operation.
/// 
/// [BitSet] and intersection operations are guaranteed to never return empty blocks
/// during iteration. 
/// 
/// TODO: consider changing this behavior.
/// 
/// [BitSet]: crate::BitSet
pub trait BlockIterator
    : Iterator<Item = DataBlock<Self::DataBitBlock>> 
    + Sized
{
    // TODO: DataBlock/Config instead?
    type DataBitBlock: BitBlock;

    /// Construct cursor for BlockIterator, with current position.
    fn cursor(&self) -> BlockIterCursor;

    type IndexIter: IndexIterator<BlockIter = Self>;

    /// Into index iterator.
    fn as_indices(self) -> Self::IndexIter;

    /// Move iterator to cursor position.
    /// 
    /// Fast O(1) operation.
    fn move_to(self, cursor: BlockIterCursor) -> Self;
}

/// Index iterator.
pub trait IndexIterator
    : Iterator<Item = usize>
    + Sized
{
    type BlockIter: BlockIterator;

    /// Into block iterator.
    fn as_blocks(self) -> Self::BlockIter;

    fn cursor(&self) -> IndexIterCursor;

    /// Move iterator to cursor position.
    /// 
    /// Fast O(1) operation.
    fn move_to(self, cursor: IndexIterCursor) -> Self;
}

// TODO: Remove this, or move to simple_iter
// It's just flatmap across block iterator.
#[cfg(feature = "simple_iter")]
pub struct IndexIter<T>
where
    T: BlockIterator
{
    block_iter: T,
    data_block_iter: DataBlockIter<T::DataBitBlock>,
}
#[cfg(feature = "simple_iter")]
impl<T> IndexIter<T>
where
    T: BlockIterator
{
    #[inline]
    pub fn new(block_iter: T) -> Self{
        Self{
            block_iter,
            data_block_iter: DataBlockIter::empty()
        }
    }
}
#[cfg(feature = "simple_iter")]
impl<T> IndexIterator for IndexIter<T>
where
    T: BlockIterator
{
    type BlockIter = T;

    #[inline]
    fn as_blocks(self) -> Self::BlockIter{
        self.block_iter
    }

    fn cursor(&self) -> IndexIterCursor {
        unimplemented!()     
    }

    fn move_to(self, _cursor: IndexIterCursor) -> Self {
        unimplemented!()
    }
}
#[cfg(feature = "simple_iter")]
impl<T> Iterator for IndexIter<T>
where
    T: BlockIterator
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