//! Iteration always return ordered (or sorted) index sequences.

use std::marker::PhantomData;
use std::mem;
use std::ops::ControlFlow;

use crate::block::Block;
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
/// Created by [BlockIterator::cursor()], used by [BlockIterator::move_to()].
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
    /// Iterator [moved_to] this cursor will always return `None`. 
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
/// Created by [IndexIterator::cursor()], used by [IndexIterator::move_to()].
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
    /// Iterator [moved_to] this cursor will always return `None`. 
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

// TODO: move inside caching iterator
pub(crate) struct State<Conf: Config> {
    pub(crate) level0_iter: <Conf::Level0BitBlock as BitBlock>::BitsIter,
    pub(crate) level1_iter: <Conf::Level1BitBlock as BitBlock>::BitsIter,
    pub(crate) level0_index: usize,
}

impl<Conf: Config> Clone for State<Conf>{
    #[inline]
    fn clone(&self) -> Self {
        Self { 
            level0_iter: self.level0_iter.clone(), 
            level1_iter: self.level1_iter.clone(), 
            level0_index: self.level0_index.clone() 
        }
    }
}

/// Block iterator.
/// 
/// # Empty blocks
/// 
/// Block iterator may occasionally return empty blocks.
/// This is for performance reasons - it is faster to just iterate/traverse empty
/// blocks through, then to add adding additional `is_empty` check in the middle of the loop.
/// 
/// TODO: consider changing this behavior.
/// 
/// [BitSet]: crate::BitSet
pub trait BlockIterator
    : Iterator<Item = DataBlock<<Self::Conf as Config>::DataBitBlock>> 
    + Sized
{
    type Conf: Config;

    /// Constructs cursor for BlockIterator, with current iterator position.
    /// 
    /// This means that if you [move_to] iterator to cursor, 
    /// iterator will be in the same position as now. IOW - cursor points
    /// to the NEXT element.
    fn cursor(&self) -> BlockCursor<Self::Conf>;

    type IndexIter: IndexIterator;

    /// Into index iterator.
    fn into_indices(self) -> Self::IndexIter;

    /// Move iterator to cursor position.
    /// 
    /// Fast O(1) operation.
    fn move_to(self, cursor: BlockCursor<Self::Conf>) -> Self;

    /// Stable [try_for_each] version.
    /// 
    /// [try_for_each]: std::iter::Iterator::try_for_each
    fn traverse<F>(self, f: F) -> ControlFlow<()>
    where
        F: FnMut(DataBlock<<Self::Conf as Config>::DataBitBlock>) -> ControlFlow<()>;
}

/// Index iterator.
pub trait IndexIterator
    : Iterator<Item = usize>
    + Sized
{
    type Conf: Config;

    /// Same as [BlockIterator::cursor], but for index.
    fn cursor(&self) -> IndexCursor<Self::Conf>;

    /// Move iterator to cursor position.
    /// 
    /// Fast O(1) operation.
    fn move_to(self, cursor: IndexCursor<Self::Conf>) -> Self;

    /// Stable [try_for_each] version.
    /// 
    /// [try_for_each]: std::iter::Iterator::try_for_each
    fn traverse<F>(self, f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>;
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
    fn cursor(&self) -> IndexCursor {
        unimplemented!()     
    }

    fn move_to(self, _cursor: IndexCursor) -> Self {
        unimplemented!()
    }

    fn traverse<F>(self, f: F) -> ControlFlow<()> where F: FnMut(usize) -> ControlFlow<()> {
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