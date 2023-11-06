use std::any::TypeId;
use std::marker::PhantomData;
use crate::{IConfig, LevelMasks};
use crate::binary_op::{BinaryOp, BitAndOp};
use crate::cache::{CacheStorage, CacheStorageBuilder};
use crate::iter::{IterExt3, SimpleIter};
use crate::virtual_bitset::{LevelMasksExt3, LevelMasksRef};

#[derive(Clone)]
pub struct Reduce<Op, S, Storage> {
    pub(crate) sets: S,
    pub(crate) phantom: PhantomData<(Op, Storage)>
}

impl<Op, S, Storage> Reduce<Op, S, Storage>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasks,
    Storage: CacheStorageBuilder
{
    // TODO: This is BLOCK iterator. Make separate iterator for usizes.
    // TODO: Benchmark if there is need for "traverse".
    #[inline]
    pub fn iter(self) -> SimpleIter<Self> {
        SimpleIter::new(self)
    }

    #[inline]
    pub fn iter_ext3(self) -> IterExt3<Self>
    where
        S::Item: LevelMasksExt3,
        S: ExactSizeIterator
    {
        IterExt3::new(self)
    }
}

impl<Op, S, Storage> LevelMasks for Reduce<Op, S, Storage>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasks,
    Storage: CacheStorageBuilder
{
    type Config = <S::Item as LevelMasks>::Config;

    #[inline]
    fn level0_mask(&self) -> <Self::Config as IConfig>::Level0BitBlock {
        unsafe{
            self.sets.clone()
            .map(|set| set.level0_mask())
            .reduce(Op::hierarchy_op)
            .unwrap_unchecked()
        }
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize)
        -> <Self::Config as IConfig>::Level1BitBlock
    {
        unsafe{
            self.sets.clone()
            .map(|set| {
                set.level1_mask(level0_index)
            })
            .reduce(Op::hierarchy_op)
            .unwrap_unchecked()
        }
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize)
        -> <Self::Config as IConfig>::DataBitBlock
    {
        unsafe{
            self.sets.clone()
            .map(|set| {
                set.data_mask(level0_index, level1_index)
            })
            .reduce(Op::data_op)
            .unwrap_unchecked()
        }
    }
}

impl<Op, S, Storage> LevelMasksExt3 for Reduce<Op, S, Storage>
where
    Op: BinaryOp,
    S: ExactSizeIterator + Clone,
    S::Item: LevelMasksExt3,
    Storage: CacheStorageBuilder
{
    type Level1Blocks3 = (
        // array of S::LevelMasksExt3
        <Storage as CacheStorageBuilder>::Storage<<S::Item as LevelMasksExt3>::Level1Blocks3>,
        // len
        usize
    );

    const EMPTY_LVL1_TOLERANCE: bool = false;

    #[inline]
    fn make_level1_blocks3(&self) -> Self::Level1Blocks3 {
        // It should be faster to calculate sets amount in front,
        // then to relocated Vec with pushes during DynamicCache construction.
        let sets_count = || self.sets.clone().count();

        let mut storage: <Storage as CacheStorageBuilder>::Storage<<S::Item as LevelMasksExt3>::Level1Blocks3>
            = Storage::build(sets_count);

        // init storage in deep
        unsafe{
            let mut index = 0;
            let elements = storage.as_mut_ptr();
            for set in self.sets.clone() {
                let element = elements.add(index);
                std::ptr::write(
                    element,
                    set.make_level1_blocks3()
                );
                index += 1;
            }
            assert!(Storage::FIXED_CAPACITY >= index, "Reduce cache overflow");
        }

        return (storage, 0);
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        &self, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        let (level1_blocks_storage, level1_blocks_len) = level1_blocks;
        let level1_blocks_ptr = level1_blocks_storage.as_mut_ptr();

        // This should act the same as a few assumes in default loop,
        // but I feel safer this way.
        if TypeId::of::<Op>() == TypeId::of::<BitAndOp>() { /* compile-time check */
            // intersection case can be optimized, since we know
            // that with intersection, there can be no
            // empty masks/blocks queried.
            let mut index = 0;
            let mask =
                self.sets.clone()
                .map(|set|{
                    let (mask, valid) = set.update_level1_blocks3(
                        &mut *level1_blocks_ptr.add(index),
                        level0_index
                    );
                    // assume(valid)
                    if !valid{ std::hint::unreachable_unchecked(); }
                    index += 1;
                    mask
                })
                .reduce(Op::hierarchy_op)
                .unwrap_unchecked();

            *level1_blocks_len = index;
            return (mask, true);
        }

        // Overwrite only non-empty blocks.
        let mut index = 0;

        let mask_acc =
            self.sets.clone()
            .map(|set|{
                let (level1_mask, valid) = set.update_level1_blocks3(
                    &mut *level1_blocks_ptr.add(index),
                    level0_index
                );
                index += valid as usize;
                level1_mask
            })
            .reduce(Op::hierarchy_op)
            .unwrap_unchecked();

        *level1_blocks_len = index;
        (mask_acc, index !=0)
    }

    #[inline]
    unsafe fn data_mask_from_blocks3(
        /*&self, */level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock {
        unsafe{
            let slice = std::slice::from_raw_parts(
                level1_blocks.0.as_ptr(),
                level1_blocks.1
            );

            slice.iter()
                .map(|set_level1_blocks|
                    <S::Item as LevelMasksExt3>::data_mask_from_blocks3(
                        set_level1_blocks, level1_index
                    )
                )
                .reduce(Op::data_op)
                // level1_blocks can not be empty, since then -
                // level1 mask will be empty, and there will be nothing to iterate.
                .unwrap_unchecked()
        }
    }
}

impl<Op, S, Storage> LevelMasksRef for Reduce<Op, S, Storage>{}