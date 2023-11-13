//! Only &HiSparseBitset, Op, &Op, Reduce, &Reduce implements
//! [LevelMasksExt]. This guarantees that [DataBitBlock] pointers
//! will never be invalidated during virtual bitset iteration.

use std::mem::{ManuallyDrop, MaybeUninit};
use crate::IConfig;
use crate::iter::{CachingBlockIter, BlockIterator};

pub trait LevelMasks{
    type Config: IConfig;

    fn level0_mask(&self) -> <Self::Config as IConfig>::Level0BitBlock;

    /// # Safety
    ///
    /// index is not checked
    unsafe fn level1_mask(&self, level0_index: usize)
        -> <Self::Config as IConfig>::Level1BitBlock;

    /// # Safety
    ///
    /// indices are not checked
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize)
        -> <Self::Config as IConfig>::DataBitBlock;
}

pub trait LevelMasksExt3: LevelMasks{
    /// Consists from child caches + Self state.
    /// Fot internal use (ala state).
    type CacheData;

    /// Cached Level1Blocks3 for faster accessing DataBlocks,
    /// without traversing whole hierarchy for getting each block during iteration.
    ///
    /// This may have less elements then sets size, because empty can be skipped.
    ///
    /// Must be POD. (Drop will not be called)
    type Level1Blocks3;

    /// Could [data_mask_from_blocks3] be called if [update_level1_blocks3]
    /// returned false.
    ///
    /// Mainly used by op.
    const EMPTY_LVL1_TOLERANCE: bool;

    fn make_cache(&self) -> Self::CacheData;

    /// Having separate function for drop not strictly necessary, since
    /// CacheData can actually drop itself. But! This allows not to store cache
    /// size within CacheData. Which makes FixedCache CacheData ZST, if its childs
    /// ZST, and which makes cache construction and destruction noop. Which is
    /// important for short iteration sessions.
    fn drop_cache(&self, cache: &mut ManuallyDrop<Self::CacheData>);

    /// Update `level1_blocks` and
    /// return (Level1Mask, is_not_empty/valid).
    ///
    /// if level0_index valid - update `level1_blocks`.
    unsafe fn update_level1_blocks3(
        &self,
        cache: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks3>,
        level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool);

    /// # Safety
    ///
    /// - indices are not checked
    /// - if ![EMPTY_LVL1_TOLERANCE] should not be called, if
    ///   [update_level1_blocks3] returned false.
    unsafe fn data_mask_from_blocks3(
        /*&self,*/ level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock;
}

/// Marker trait, for implementing LevelMasks for &impl LevelMasks.
///
/// This also prevents nested &&&&& auto-implementations.
pub(crate) trait LevelMasksRef{}

impl<'a, T: LevelMasks + LevelMasksRef> LevelMasks for &'a T {
    type Config = T::Config;

    #[inline]
    fn level0_mask(&self) -> <Self::Config as IConfig>::Level0BitBlock {
        <T as LevelMasks>::level0_mask(self)
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize)
        -> <Self::Config as IConfig>::Level1BitBlock
    {
        <T as LevelMasks>::level1_mask(self, level0_index)
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize)
        -> <Self::Config as IConfig>::DataBitBlock
    {
        <T as LevelMasks>::data_mask(self, level0_index, level1_index)
    }
}

impl<'a, T: LevelMasksExt3 + LevelMasksRef> LevelMasksExt3 for &'a T {
    type Level1Blocks3 = T::Level1Blocks3;

    const EMPTY_LVL1_TOLERANCE: bool = T::EMPTY_LVL1_TOLERANCE;

    type CacheData = T::CacheData;

    #[inline]
    fn make_cache(&self) -> Self::CacheData {
        <T as LevelMasksExt3>::make_cache(self)
    }

    #[inline]
    fn drop_cache(&self, cache: &mut ManuallyDrop<Self::CacheData>) {
        <T as LevelMasksExt3>::drop_cache(self, cache)
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        &self,
        cache_data: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks3>,
        level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        <T as LevelMasksExt3>::update_level1_blocks3(
            self, cache_data, level1_blocks, level0_index
        )
    }

    #[inline]
    unsafe fn data_mask_from_blocks3(
        level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock {
        <T as LevelMasksExt3>::data_mask_from_blocks3(
            level1_blocks, level1_index
        )
    }
}


// TODO: rename to IBitSet? / BitSetInterface
pub trait VirtualBitSet{
    type BlockIter: Iterator;
    fn block_iter(self) -> Self::BlockIter;

    type Iter: Iterator;
    fn iter(self) -> Self::Iter;

    fn contains(&self, index: usize) -> bool;
}

impl<T:LevelMasksExt3> VirtualBitSet for T{
    type BlockIter = <T::Config as IConfig>::DefaultBlockIterator<T>;

    #[inline]
    fn block_iter(self) -> Self::BlockIter {
        BlockIterator::new(self)
    }

    type Iter = <Self::BlockIter as BlockIterator>::IndexIter;

    #[inline]
    fn iter(self) -> Self::Iter {
        self.block_iter().as_indices()
    }

    fn contains(&self, index: usize) -> bool {
        todo!()
    }
}