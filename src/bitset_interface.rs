use std::mem::{ManuallyDrop, MaybeUninit};
use crate::{BitSet, IConfig, level_indices};
use crate::binary_op::BinaryOp;
use crate::bit_block::BitBlock;
use crate::cache::ReduceCache;
use crate::configs::DefaultBlockIterator;
use crate::iter::BlockIterator;
use crate::op::BitSetOp;
use crate::reduce::Reduce;

/// Basic interface for accessing block masks. Can work with [SimpleIter].
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


/// More sophisticated masks interface, optimized for iteration speed, through
/// caching level1(pre-data level) block pointer. This also, allow to discard
/// sets with empty level1 blocks in final stage of getting data blocks.
///
/// For use with [CachingIter].
pub trait LevelMasksExt: LevelMasks{
    /// Consists from child caches + Self state.
    /// Fot internal use (ala state).
    type CacheData;

    /// Cached Level1Blocks for faster accessing DataBlocks,
    /// without traversing whole hierarchy for getting each block during iteration.
    ///
    /// This may have less elements then sets size, because empty can be skipped.
    ///
    /// Must be POD. (Drop will not be called)
    type Level1Blocks;

    /// Could [data_mask_from_blocks3] be called if [update_level1_blocks3]
    /// returned false.
    ///
    /// Mainly used by op.
    const EMPTY_LVL1_TOLERANCE: bool;

    fn make_cache(&self) -> Self::CacheData;

    /// Having separate function for drop not strictly necessary, since
    /// CacheData can actually drop itself. But! This allows not to store cache
    /// size within CacheData. Which makes FixedCache CacheData ZST, if its childs
    /// are ZSTs, and which makes cache construction and destruction noop. Which is
    /// important for short iteration sessions.
    fn drop_cache(&self, cache: &mut ManuallyDrop<Self::CacheData>);

    /// Update `level1_blocks` and
    /// return (Level1Mask, is_not_empty/valid).
    ///
    /// if level0_index valid - update `level1_blocks`.
    unsafe fn update_level1_blocks(
        &self,
        cache: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks>,
        level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool);

    /// # Safety
    ///
    /// - indices are not checked
    /// - if ![EMPTY_LVL1_TOLERANCE] should not be called, if
    ///   [update_level1_blocks] returned false.
    unsafe fn data_mask_from_blocks(
        /*&self,*/ level1_blocks: &Self::Level1Blocks, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock;
}

impl<'a, T: LevelMasks> LevelMasks for &'a T {
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

impl<'a, T: LevelMasksExt> LevelMasksExt for &'a T {
    type Level1Blocks = T::Level1Blocks;

    const EMPTY_LVL1_TOLERANCE: bool = T::EMPTY_LVL1_TOLERANCE;

    type CacheData = T::CacheData;

    #[inline]
    fn make_cache(&self) -> Self::CacheData {
        <T as LevelMasksExt>::make_cache(self)
    }

    #[inline]
    fn drop_cache(&self, cache: &mut ManuallyDrop<Self::CacheData>) {
        <T as LevelMasksExt>::drop_cache(self, cache)
    }

    #[inline]
    unsafe fn update_level1_blocks(
        &self,
        cache_data: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks>,
        level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        <T as LevelMasksExt>::update_level1_blocks(
            self, cache_data, level1_blocks, level0_index
        )
    }

    #[inline]
    unsafe fn data_mask_from_blocks(
        level1_blocks: &Self::Level1Blocks, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock {
        <T as LevelMasksExt>::data_mask_from_blocks(
            level1_blocks, level1_index
        )
    }
}


// User-side interface
pub trait BitSetInterface: IntoIterator<Item = usize> + LevelMasksExt{
    type Config: IConfig;

    type BlockIter<'a>: BlockIterator where Self: 'a;
    fn block_iter(&self) -> Self::BlockIter<'_>;

    type Iter<'a>: Iterator<Item = usize> where Self: 'a;
    fn iter(&self) -> Self::Iter<'_>;

    type IntoBlockIter: BlockIterator;
    fn into_block_iter(self) -> Self::IntoBlockIter;

    fn contains(&self, index: usize) -> bool;
}

impl<T: LevelMasksExt> BitSetInterface for T
where
    T: IntoIterator<Item = usize>
{
    type Config = T::Config;
    type BlockIter<'a> = DefaultBlockIterator<&'a T> where Self: 'a;

    #[inline]
    fn block_iter(&self) -> Self::BlockIter<'_> {
        BlockIterator::new(self)
    }

    type Iter<'a> = <Self::BlockIter<'a> as BlockIterator>::IndexIter where Self: 'a;

    #[inline]
    fn iter(&self) -> Self::Iter<'_> {
        self.block_iter().as_indices()
    }

    type IntoBlockIter = DefaultBlockIterator<T>;

    #[inline]
    fn into_block_iter(self) -> Self::IntoBlockIter {
        BlockIterator::new(self)
    }

    #[inline]
    fn contains(&self, index: usize) -> bool {
        let (level0_index, level1_index, data_index) = level_indices::<T::Config>(index);
        unsafe{
            let data_block = self.data_mask(level0_index, level1_index);
            data_block.get_bit(data_index)
        }
    }
}


macro_rules! impl_into_iter {
    (impl <$($generics:tt),*> for $t:ty where $($where_bounds:tt)*) => {
        impl<$($generics),*> IntoIterator for $t
        where
            $($where_bounds)*
        {
            type Item = usize;
            type IntoIter = <<Self as BitSetInterface>::IntoBlockIter as BlockIterator>::IndexIter;

            #[inline]
            fn into_iter(self) -> Self::IntoIter {
                self.into_block_iter().as_indices()
            }
        }
    };
}

impl_into_iter!(impl<Config> for BitSet<Config> where Config: IConfig );
impl_into_iter!(impl<'a, Config> for &'a BitSet<Config> where Config: IConfig );
impl_into_iter!(
    impl<Op, S1, S2> for BitSetOp<Op, S1, S2>
    where
        Op: BinaryOp,
        S1: LevelMasksExt<Config = S2::Config>,
        S2: LevelMasksExt
);
impl_into_iter!(
    impl<'a, Op, S1, S2> for &'a BitSetOp<Op, S1, S2>
    where
        Op: BinaryOp,
        S1: LevelMasksExt<Config = S2::Config>,
        S2: LevelMasksExt
);
impl_into_iter!(
    impl<Op, S, Storage> for Reduce<Op, S, Storage>
    where
        Op: BinaryOp,
        S: Iterator + Clone,
        S::Item: LevelMasksExt,
        Storage: ReduceCache
);
impl_into_iter!(
    impl<'a, Op, S, Storage> for &'a Reduce<Op, S, Storage>
    where
        Op: BinaryOp,
        S: Iterator + Clone,
        S::Item: LevelMasksExt,
        Storage: ReduceCache
);