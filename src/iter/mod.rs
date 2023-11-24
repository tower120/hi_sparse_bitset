//! Iteration always return ordered (or sorted) index sequences.

use crate::{DataBlock, DataBlockIter};
use crate::bit_block::BitBlock;
use crate::bitset_interface::{BitSetBase, LevelMasksExt};
use crate::config::IConfig;

mod caching;
pub use caching::{CachingBlockIter, CachingIndexIter};

#[cfg(feature = "simple_iter")]
mod simple;
#[cfg(feature = "simple_iter")]
pub use simple::{SimpleBlockIter, SimpleIndexIter};

#[inline]
fn data_block_start_index<Config: IConfig>(level0_index: usize, level1_index: usize) -> usize{
    let level0_offset = level0_index << (Config::DataBitBlock::SIZE_POT_EXPONENT + Config::Level1BitBlock::SIZE_POT_EXPONENT);
    let level1_offset = level1_index << (Config::DataBitBlock::SIZE_POT_EXPONENT);
    level0_offset + level1_offset
}

// TODO: Looks like Cursor for IndexIter possible too. Do we need it?
/// Iterator cursor, or position of iterable.
/// 
/// Created by [BlockIterator::cursor()], consumed by [BlockIterator::skip_to()].
/// 
/// Allows to resume iteration from the last position, even if the
/// source was mutated. Can be used with any [BitSetInterface].
/// Default constructed State will traverse bitset from the very begin.
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
    pub(crate) level1_index: usize,
}

pub(crate) struct State<Config: IConfig> {
    pub(crate) level0_iter: <Config::Level0BitBlock as BitBlock>::BitsIter,
    pub(crate) level1_iter: <Config::Level1BitBlock as BitBlock>::BitsIter,
    pub(crate) level0_index: usize,
}

pub trait BlockIterator
    : Iterator<Item = DataBlock<
        <<Self::BitSet as BitSetBase>::Config as IConfig>::DataBitBlock
    >>
    + Sized
{
    // TODO: Do we even need these two in public interface?
    // TODO: rename latter
    type BitSet: LevelMasksExt;
    fn new(virtual_set: Self::BitSet) -> Self;

    /// Constructs cursor for BlockIterator, with current position.
    fn cursor(&self) -> BlockIterCursor;

    type IndexIter: IndexIterator<BlockIter = Self>;

    /// Into index iterator.
    fn as_indices(self) -> Self::IndexIter;

    // TODO: rename to advance_to ?
    /// Advance iterator to cursor position. If iterator is past
    /// the cursor - have no effect.
    fn skip_to(&mut self, cursor: BlockIterCursor);
}

// TODO: We have common implementation?
pub trait IndexIterator
    : Iterator<Item = usize>
    + Sized
{
    type BlockIter: BlockIterator;

    /// Into block iterator.
    fn as_blocks(self) -> Self::BlockIter;
}

// It's just flatmap across block iterator.
pub struct IndexIter<T>
where
    T: BlockIterator
{
    block_iter: T,
    data_block_iter: DataBlockIter<<<T::BitSet as BitSetBase>::Config as IConfig>::DataBitBlock>,
}

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

impl<T> IndexIterator for IndexIter<T>
where
    T: BlockIterator
{
    type BlockIter = T;

    #[inline]
    fn as_blocks(self) -> Self::BlockIter{
        self.block_iter
    }
}

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