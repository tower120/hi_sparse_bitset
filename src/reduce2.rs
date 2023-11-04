use std::any::TypeId;
use std::marker::PhantomData;
use arrayvec::ArrayVec;
use crate::{IConfig, LevelMasks};
use crate::binary_op::{BinaryOp, BitAndOp};
use crate::iter::{IterExt3, SimpleIter};
use crate::virtual_bitset::{LevelMasksExt3, LevelMasksRef};
const MAX_SETS: usize = 32;
#[derive(Clone)]
pub struct Reduce<Op, S>
/*where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasks,*/
{
    pub(crate) sets: S,
    pub(crate) phantom: PhantomData<Op>
}

impl<Op, S> Reduce<Op, S>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasks,
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


impl<Op, S> LevelMasks for Reduce<Op, S>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasks,
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

impl<Op, S> LevelMasksExt3 for Reduce<Op, S>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasksExt3,
{
    // TODO: Use [_; MAX_SETS] with len, for better predictability.
    //       ArrayVec is NOT guaranteed to be POD.
    //       (thou, current implementation is)
    type Level1Blocks3 = ArrayVec<<S::Item as LevelMasksExt3>::Level1Blocks3, MAX_SETS>;

    #[inline]
    fn make_level1_blocks3(&self) -> Self::Level1Blocks3 {
        // Basically do nothing.
        let mut array = ArrayVec::new();
        unsafe {
            // calling constructors in deep
            for (index, set) in self.sets.clone().enumerate() {
                std::ptr::write(
                    array.get_unchecked_mut(index),
                    set.make_level1_blocks3()
                );
            }
            // len need to be set on every "update" anyway
            //array.set_len(self.sets.len());
        }
        array
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        &self, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
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
                        level1_blocks.get_unchecked_mut(index),
                        level0_index
                    );
                    // assume(valid)
                    if !valid{ std::hint::unreachable_unchecked(); }
                    index += 1;
                    mask
                })
                .reduce(Op::hierarchy_op)
                .unwrap_unchecked();

            // Contradictory this have no effect in benchmarks.
            //level1_blocks.set_len(self.sets.len());

            level1_blocks.set_len(index);
            return (mask, true);
        }

        // Overwrite only non-empty blocks.
        let mut level1_blocks_index = 0;

        let mask_acc =
            self.sets.clone()
            .map(|set|{
                let (level1_mask, valid) = set.update_level1_blocks3(
                    level1_blocks.get_unchecked_mut(level1_blocks_index),
                    level0_index
                );
                level1_blocks_index += valid as usize;
                level1_mask
            })
            .reduce(Op::hierarchy_op)
            .unwrap_unchecked();

        level1_blocks.set_len(level1_blocks_index);
        (mask_acc, level1_blocks_index!=0)
    }

    #[inline]
    unsafe fn data_mask_from_blocks3(
        /*&self, */level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock {
        unsafe{
            level1_blocks.iter()
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

impl<Op, S> LevelMasksRef for Reduce<Op, S>{}