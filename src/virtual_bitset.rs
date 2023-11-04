//! Only &HiSparseBitset, Op, &Op, Reduce, &Reduce implements
//! [LevelMasksExt]. This guarantees that [DataBitBlock] pointers
//! will never be invalidated during virtual bitset iteration.

use crate::IConfig;

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
    /// Container/value/owned data
    ///
    /// Must be POD.
    type Level1Blocks3;

    /// Make Level1Blocks in a state that can be used in `update_level1_blocks`.
    ///
    /// For example, Level1Blocks may be in uninitialized state, if
    /// `update_level1_blocks` will initialize it any way.
    fn make_level1_blocks3(&self) -> Self::Level1Blocks3;

    /// Update `level1_blocks` and
    /// return (Level1Mask, is_not_empty/valid).
    ///
    /// if level0_index valid - update `level1_blocks`.
    unsafe fn update_level1_blocks3(
        &self, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool);

    /// # Safety
    ///
    /// indices are not checked
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

    #[inline]
    fn make_level1_blocks3(&self) -> Self::Level1Blocks3 {
        <T as LevelMasksExt3>::make_level1_blocks3(self)
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        &self, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        <T as LevelMasksExt3>::update_level1_blocks3(
            self, level1_blocks, level0_index
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