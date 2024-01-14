//! Iteration always return ordered (or sorted) index sequences.

use std::marker::PhantomData;
use std::mem;

use crate::{DataBlock, DataBlockIter, level_indices};
use crate::bit_block::BitBlock;
use crate::config::Config;

mod caching;
pub use caching::{CachingBlockIter, CachingIndexIter};

#[cfg(feature = "simple_iter")]
mod simple;
#[cfg(feature = "simple_iter")]
pub use simple::{SimpleBlockIter, SimpleIndexIter};

/// Block iterator cursor, or position of iterable.
/// 
/// Created by [CachingBlockIter::cursor()], used by [CachingBlockIter::move_to()].
/// Also can be built [from] index and DataBlock.
/// 
/// [from]: Self::from
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
/// buffer, take iterator to cursor, unlock sets, process buffer, lock sets,
/// move iterator to cursor, and so on.
/// 
/// [BitSetInterface]: crate::BitSetInterface
//
// Additional `Conf` generic argument helps with index safety, and enhance 
// type safety.
pub struct BlockCursor<Conf: Config> {
    pub(crate) level0_index: u16,
    // We don't have current/last returned index in iterator
    pub(crate) level1_next_index: u16,
    pub(crate) phantom: PhantomData<Conf>
}

impl<Conf: Config> Default for BlockCursor<Conf>{
    #[inline]
    fn default() -> Self {
        Self::start()
    }
}

impl<Conf: Config> BlockCursor<Conf>{
    /// Constructs cursor that points to the start of bitset.
    #[inline]
    pub fn start() -> Self{
        unsafe{ std::mem::zeroed() }
    }
    
    /// Constructs cursor that points to the end of bitset.
    ///
    /// Iterator [moved to] this cursor will always return `None`. 
    /// 
    /// [moved to]: CachingBlockIter::move_to
    #[inline]
    pub fn end() -> Self{
        Self{
            level0_index: Conf::Level0BitBlock::size() as u16,
            level1_next_index: Conf::Level1BitBlock::size() as u16,
            phantom: Default::default(),
        }
    }   
}

impl<Conf: Config> Clone for BlockCursor<Conf>{
    #[inline]
    fn clone(&self) -> Self {
        unsafe{ std::ptr::read(self) }
    }
}
impl<Conf: Config> Copy for BlockCursor<Conf>{}

impl<Conf: Config> From<usize> for BlockCursor<Conf>{
    /// Build cursor that points to the block, that contains `index`.
    #[inline]
    fn from(mut index: usize) -> Self {
        index = std::cmp::min(index, Conf::max_value());

        let (level0, level1, _) = level_indices::<Conf>(index);
        Self{
            level0_index: level0 as u16,
            level1_next_index: level1 as u16,
            phantom: PhantomData,
        }
    }
}

impl<Conf: Config> From<&DataBlock<Conf::DataBitBlock>> for BlockCursor<Conf>{
    /// Build cursor that points to the `block`.
    #[inline]
    fn from(block: &DataBlock<Conf::DataBitBlock>) -> Self {
        Self::from(block.start_index)
    }
}

/// Index iterator cursor.
/// 
/// Created by [CachingIndexIter::cursor()], used by [CachingIndexIter::move_to()].
/// Also can be built [from] index and DataBlock.
/// 
/// [from]: Self::from
/// 
/// Same as [BlockCursor], but for indices iterator.
pub struct IndexCursor<Conf: Config> {
    pub(crate) block_cursor: BlockCursor<Conf>,
    // use u32 instead of u16, to nicely fit 64bit register
    pub(crate) data_next_index: u32
}

impl<Conf: Config> Default for IndexCursor<Conf>{
    #[inline]
    fn default() -> Self {
        Self::start()
    }
}

impl<Conf: Config> IndexCursor<Conf>{
    /// Constructs cursor that points to the start of the bitset.
    #[inline]
    pub fn start() -> Self{
        unsafe{ std::mem::zeroed() }
    }
    
    /// Constructs cursor that points to the end of the bitset.
    ///
    /// Iterator [moved to] this cursor will always return `None`. 
    /// 
    /// [moved to]: CachingIndexIter::move_to
    #[inline]
    pub fn end() -> Self{
        Self{
            block_cursor: BlockCursor::end(),
            data_next_index: Conf::DataBitBlock::size() as u32
        }
    }   
}

impl<Conf: Config> Clone for IndexCursor<Conf>{
    #[inline]
    fn clone(&self) -> Self {
        unsafe{ std::ptr::read(self) }
    }
}
impl<Conf: Config> Copy for IndexCursor<Conf>{}

impl<Conf: Config> From<usize> for IndexCursor<Conf>{
    /// Build cursor that points to the `index`.
    #[inline]
    fn from(mut index: usize) -> Self {
        index = std::cmp::min(index, Conf::max_value());

        let (level0, level1, data) = level_indices::<Conf>(index);
        Self{
            block_cursor: BlockCursor { 
                level0_index: level0 as u16,
                level1_next_index: level1 as u16,
                phantom: PhantomData
            },
            data_next_index: data as u32,
        }        
    }
}

impl<Conf: Config> From<&DataBlock<Conf::DataBitBlock>> for IndexCursor<Conf>{
    /// Build cursor that points to the `block` start index.
    #[inline]
    fn from(block: &DataBlock<Conf::DataBitBlock>) -> Self {
        Self::from(block.start_index)
    }
}