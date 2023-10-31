//! Only &HiSparseBitset, Op, &Op, Reduce, &Reduce implements
//! [LevelMasksExt]. This guarantees that [DataBitBlock] pointers
//! will never be invalidated during virtual bitset iteration.
// TODO: leave only refs?

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

pub trait LevelMasksExt: LevelMasks{
    /// Container/value/owned data
    type Level1Blocks;

    /// Make Level1Blocks in a state that can be used in `update_level1_blocks`.
    ///
    /// For example, Level1Blocks may be in uninitialized state, if
    /// `update_level1_blocks` will initialize it any way.
    fn make_level1_blocks(&self) -> Self::Level1Blocks;

    /// Level1Blocks should be fully initialized after calling this function.
    ///
    /// # Safety
    ///
    /// index is not checked
    unsafe fn update_level1_blocks(
        &self, level1_blocks: &mut Self::Level1Blocks, level0_index: usize
    );

    /// # Safety
    ///
    /// indices are not checked
    unsafe fn data_mask_from_blocks(
        &self, level1_blocks: &Self::Level1Blocks, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock;
}


pub trait LevelMasksExt2: LevelMasks{
    /// Container/value/owned data
    ///
    /// Must be POD.
    type Level1Blocks2;

    /// Make Level1Blocks in a state that can be used in `update_level1_blocks`.
    ///
    /// For example, Level1Blocks may be in uninitialized state, if
    /// `update_level1_blocks` will initialize it any way.
    fn make_level1_blocks2(&self) -> Self::Level1Blocks2;

    /// Level1Blocks should be fully initialized after calling this function.
    ///
    /// # Safety
    ///
    /// index is not checked
    unsafe fn update_level1_blocks2 (
        &self, level1_blocks: &mut Self::Level1Blocks2, level0_index: usize
    ) -> bool /* !is_empty */;

    /// # Safety
    ///
    /// indices are not checked
    unsafe fn data_mask_from_blocks2(
        /*&self,*/ level1_blocks: &Self::Level1Blocks2, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock;
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

    /// Combined level1 block list update + level1 mask (for level0_index).
    /// Returns LevelMasks::level1_mask, or None, if level0_index invalid.
    ///
    /// Level1Blocks should be fully initialized after calling this function,
    /// if function did not return None.
    ///
    /// # Safety
    ///
    /// index is not checked
    unsafe fn update_level1_blocks3 (
        &self, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> Option<<Self::Config as IConfig>::Level1BitBlock>;

    /// Same as update_level1_blocks3 but always update level1_blocks
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