use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::BitOr;
use crate::binary_op::{BinaryOp, BitOrOp};
use crate::{HiSparseBitset, IConfig};
use crate::iter::IterExt3;
use crate::virtual_bitset::{LevelMasks, LevelMasksExt3};

pub struct HiSparseBitsetOp<Op, S1, S2>{
    pub(crate) s1: S1,
    pub(crate) s2: S2,
    pub(crate) phantom: PhantomData<Op>
}
impl<Op, S1, S2> HiSparseBitsetOp<Op, S1, S2>{
    #[inline]
    pub(crate) fn new(_:Op, s1:S1, s2:S2) -> Self{
        HiSparseBitsetOp{ s1, s2, phantom:PhantomData }
    }
}

impl<Op, S1, S2> HiSparseBitsetOp<Op, S1, S2>
where
    Op: BinaryOp,
    S1: LevelMasksExt3,
    S2: LevelMasksExt3<Config = S1::Config>,
{
    #[inline]
    pub fn iter_ext3(self) -> IterExt3<Self> {
        IterExt3::new(self)
    }
}

impl<Op, S1, S2> LevelMasks for HiSparseBitsetOp<Op, S1, S2>
where
    Op: BinaryOp,
    S1: LevelMasks,
    S2: LevelMasks<Config = S1::Config>,
{
    type Config = S1::Config;

    #[inline]
    fn level0_mask(&self) -> <Self::Config as IConfig>::Level0BitBlock {
        Op::hierarchy_op(self.s1.level0_mask(), self.s2.level0_mask())
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize)
        -> <Self::Config as IConfig>::Level1BitBlock
    {
        Op::hierarchy_op(
            self.s1.level1_mask(level0_index),
            self.s2.level1_mask(level0_index)
        )
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize)
        -> <Self::Config as IConfig>::DataBitBlock
    {
        Op::data_op(
            self.s1.data_mask(level0_index, level1_index),
            self.s2.data_mask(level0_index, level1_index)
        )
    }
}

impl<Op, S1, S2> LevelMasksExt3 for HiSparseBitsetOp<Op, S1, S2>
where
    Op: BinaryOp,
    S1: LevelMasksExt3,
    S2: LevelMasksExt3<Config = S1::Config>,
{
    // TODO: remove bools
    type Level1Blocks3 = (S1::Level1Blocks3, S2::Level1Blocks3, bool, bool);

    #[inline]
    fn make_level1_blocks3(&self) -> Self::Level1Blocks3 {
        (self.s1.make_level1_blocks3(), self.s2.make_level1_blocks3(),
            unsafe{ MaybeUninit::uninit().assume_init() },
            unsafe{ MaybeUninit::uninit().assume_init() },
        )
    }

/*    // TODO: BENCHMARK!! Looks like this have no sense for binary op.

    #[inline]
    unsafe fn update_level1_blocks3(
        &self, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> Option<<Self::Config as IConfig>::Level1BitBlock> {
        // TODO: optimization for AND case?

        let mask0 = self.s1.update_level1_blocks3(&mut level1_blocks.0, level0_index);
        let mask1 = self.s2.update_level1_blocks3(&mut level1_blocks.1, level0_index);

        level1_blocks.2 = !mask0.is_none();
        level1_blocks.3 = !mask1.is_none();

        if let Some(m0) = mask0{
            if let Some(m1) = mask1{
                Some(Op::hierarchy_op(m0, m1))
            } else {
                None
            }
        } else {
            mask1
        }
    }*/

    #[inline]
    unsafe fn always_update_level1_blocks3(
        &self, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        let (mask1, v1) = self.s1.always_update_level1_blocks3(&mut level1_blocks.0, level0_index);
        let (mask2, v2) = self.s2.always_update_level1_blocks3(&mut level1_blocks.1, level0_index);
        let mask = Op::hierarchy_op(mask1, mask2);
        (mask, v1 | v2)
    }

    #[inline]
    unsafe fn data_mask_from_blocks3(
        level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock {
        // TODO: optimization for AND case?

        let have0 = level1_blocks.2;
        let have1 = level1_blocks.3;

        let m0 = S1::data_mask_from_blocks3(&level1_blocks.0, level1_index);
        let m1 = S2::data_mask_from_blocks3(&level1_blocks.1, level1_index);
        Op::data_op(m0, m1)


        /*if have0{
            let m0 = S1::data_mask_from_blocks3(&level1_blocks.0, level1_index);
            if have1 {
                let m1 = S2::data_mask_from_blocks3(&level1_blocks.1, level1_index);
                Op::data_op(m0, m1)
            } else {
                m0
            }
        } else {
            // We're guaranteed to have at least one block.
            debug_assert!(have1);
            S2::data_mask_from_blocks3(&level1_blocks.1, level1_index)
        }*/
    }
}



impl<'a, Config: IConfig> BitOr for &'a HiSparseBitset<Config>
{
    type Output = HiSparseBitsetOp<BitOrOp, &'a HiSparseBitset<Config>, &'a HiSparseBitset<Config>>;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        HiSparseBitsetOp::new(BitOrOp, self, rhs)
    }
}

impl<'a, Op, S1, S2, Config: IConfig>
    BitOr<&'a HiSparseBitset<Config>>
    for HiSparseBitsetOp<Op, S1, S2>
where
    Config: IConfig,
    Op:BinaryOp,
    S1: LevelMasksExt3<Config = Config>,
    S2: LevelMasksExt3<Config = Config>,
{
    type Output = HiSparseBitsetOp<BitOrOp, HiSparseBitsetOp<Op, S1, S2>, &'a HiSparseBitset<Config>>;

    #[inline]
    fn bitor(self, rhs: &'a HiSparseBitset<Config>) -> Self::Output {
        HiSparseBitsetOp::new(BitOrOp, self, rhs)
    }
}