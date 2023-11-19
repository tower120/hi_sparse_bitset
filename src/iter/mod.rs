mod caching;
mod simple;

use num_traits::AsPrimitive;
use crate::{data_block_start_index, DataBlock, DataBlockIter, IConfig};
use crate::bit_block::BitBlock;
use crate::bit_queue::BitQueue;
use crate::block::Block;
use crate::bitset_interface::{LevelMasks, LevelMasksExt};

pub use caching::{CachingBlockIter, CachingIndexIter};
pub use simple::{SimpleBlockIter, SimpleIndexIter};

// TODO: Clone -able.
// TODO: Looks like State for IndexIter possible too. Do we need it?
/// Iterator state. Acts like cursor, or position of iterable.
///
/// Allows to resume iteration from the last position, even if
/// source was mutated. Both suspending and resuming operations are very fast.
///
/// Can be used with any [BitSetInterface].
/// Default constructed State will traverse bitset from the very begin.
///
/// # Resume
///
/// After resume from State, iterator will continue iteration from where
/// it was suspended. All elements that was removed since suspension will
/// not appear in iteration sequence. Newly added elements may sporadically appear
/// in output.
///
/// IOW - you're guaranteed to have your initial sequence in valid state +
/// some new valid elements (if any was added).
///
/// ## Resume from index (?)
///
/// TODO
///
/// Iterator will be resumed from last processed block and will go forward.
/// All blocks BEFORE start position will not be iterated, all blocks AFTER will.
///
/// _IOW - resumed iterator will behave like a new one, but with 0..index blocks discarded._
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
pub struct State<Config: IConfig> {
    pub(crate) level0_iter: <Config::Level0BitBlock as BitBlock>::BitsIter,
    pub(crate) level1_iter: <Config::Level1BitBlock as BitBlock>::BitsIter,
    pub(crate) level0_index: usize,
}
impl<Config: IConfig> Default for State<Config>{
    /// Iteration will start from the very begin.
    ///
    /// It is safe to use any [BitSetInterface] with default constructed `State`.
    #[inline]
    fn default() -> Self {
        Self {
            level0_iter: BitQueue::filled(),
            level1_iter: BitQueue::empty(),
            level0_index: 0
        }
    }

    // TODO: consider returning "resume()" here back to DefaultIterator
}


pub trait BlockIterator
    : Iterator<Item = DataBlock<
        <<Self::BitSet as LevelMasks>::Config as IConfig>::DataBitBlock
    >>
    + Sized
{
    // TODO: rename latter
    type BitSet: LevelMasksExt;

    fn new(virtual_set: Self::BitSet) -> Self;

    fn resume(
        virtual_set: Self::BitSet,
        state: State<<Self::BitSet as LevelMasks>::Config>
    ) -> Self;

    fn suspend(self) -> State<<Self::BitSet as LevelMasks>::Config>;

    type IndexIter: IndexIterator<BlockIter = Self>;

    /// Into index iterator.
    fn as_indices(self) -> Self::IndexIter;
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

/// Remove non-existent elements from `state` internal iterators.
fn patch_state<T, Config, F>(
    state: &mut State<Config>, virtual_set: &T, mut level1_mask_gen: F
) where
    T: LevelMasks<Config = Config>,
    Config: IConfig,
    F: FnMut(usize) -> Config::Level1BitBlock
{
    // Level0
    let level0_mask = virtual_set.level0_mask();
    let level0_index_valid = level0_mask.get_bit(state.level0_index);
    state.level0_iter.mask_out(level0_mask.as_array_u64());

    // Level1
    if level0_index_valid {
        let level1_mask = level1_mask_gen(state.level0_index);
        state.level1_iter.mask_out(level1_mask.as_array_u64());
    } else {
        // Don't touch `level0_index`.
        // It will be updated in iterator.
        state.level1_iter  = BitQueue::empty();
    }
}



// It's just flatmap across block iterator.
pub struct IndexIter<T>
where
    T: BlockIterator
{
    block_iter: T,
    data_block_iter: DataBlockIter<<<T::BitSet as LevelMasks>::Config as IConfig>::DataBitBlock>,
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