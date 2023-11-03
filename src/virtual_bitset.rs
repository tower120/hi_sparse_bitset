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

    // TODO: rename
    /// Same as update_level1_blocks3 but always update level1_blocks
    ///
    /// Return (Level1Mask, is_not_empty/valid)
    ///
    /// if level0_index valid - update `level1_blocks`.
    unsafe fn always_update_level1_blocks3 (
        &self, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool);

    /// # Safety
    ///
    /// indices are not checked
    unsafe fn data_mask_from_blocks3(
        /*&self,*/ level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock;
}