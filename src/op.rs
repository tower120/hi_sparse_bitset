use std::any::TypeId;
use std::marker::PhantomData;
use std::ops::{BitOr, BitAnd, BitXor, Sub};
use crate::binary_op::*;
use crate::{HiSparseBitset, IConfig};
use crate::bit_block::BitBlock;
use crate::iter::IterExt3;
use crate::reduce2::Reduce;
use crate::virtual_bitset::{LevelMasks, LevelMasksExt3, LevelMasksRef};

// TODO: rename to something shorter?
#[derive(Clone)]
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
    type Level1Blocks3 = (S1::Level1Blocks3, S2::Level1Blocks3, bool, bool);

    const EMPTY_LVL1_TOLERANCE: bool = true;

    #[inline]
    fn make_level1_blocks3(&self) -> Self::Level1Blocks3 {
        (self.s1.make_level1_blocks3(), self.s2.make_level1_blocks3(), false, false)
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        &self, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        let (mask1, v1) = self.s1.update_level1_blocks3(&mut level1_blocks.0, level0_index);
        let (mask2, v2) = self.s2.update_level1_blocks3(&mut level1_blocks.1, level0_index);

        let IS_INTERSECTION = TypeId::of::<Op>() == TypeId::of::<BitAndOp>();
        if !IS_INTERSECTION{
        if !S1::EMPTY_LVL1_TOLERANCE {
            level1_blocks.2 = v1;
        }
        if !S2::EMPTY_LVL1_TOLERANCE {
            level1_blocks.3 = v2;
        }
        }

        let mask = Op::hierarchy_op(mask1, mask2);
        (mask, v1 | v2)
    }

    #[inline]
    unsafe fn data_mask_from_blocks3(
        level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock {
        // intersection can never point to empty blocks.
        let IS_INTERSECTION = TypeId::of::<Op>() == TypeId::of::<BitAndOp>();

        let m0 = if S1::EMPTY_LVL1_TOLERANCE | IS_INTERSECTION | level1_blocks.2{
            S1::data_mask_from_blocks3(&level1_blocks.0, level1_index)
        } else {
            <Self::Config as IConfig>::DataBitBlock::zero()
        };

        let m1 = if S2::EMPTY_LVL1_TOLERANCE | IS_INTERSECTION | level1_blocks.3{
            S2::data_mask_from_blocks3(&level1_blocks.1, level1_index)
        } else {
            <Self::Config as IConfig>::DataBitBlock::zero()
        };

        Op::data_op(m0, m1)
    }
}

impl<Op, S1, S2> LevelMasksRef for HiSparseBitsetOp<Op, S1, S2>{}

// TODO: move behind default feature flag operations impl?
// We need this all because RUST still does not support template/generic specialization.
macro_rules! impl_op {
    ($op_class:ident, $op_fn:ident, $binary_op:ident) => {
        impl<'a, Config: IConfig, Rhs> $op_class<Rhs> for &'a HiSparseBitset<Config> {
            type Output = HiSparseBitsetOp<$binary_op, &'a HiSparseBitset<Config>, Rhs>;

            #[inline]
            fn $op_fn(self, rhs: Rhs) -> Self::Output {
                HiSparseBitsetOp::new($binary_op, self, rhs)
            }
        }

        impl<Op, S1, S2, Rhs> $op_class<Rhs> for HiSparseBitsetOp<Op, S1, S2> {
            type Output = HiSparseBitsetOp<$binary_op, HiSparseBitsetOp<Op, S1, S2>, Rhs>;

            #[inline]
            fn $op_fn(self, rhs: Rhs) -> Self::Output {
                HiSparseBitsetOp::new($binary_op, self, rhs)
            }
        }

        impl<'a, Op, S1, S2, Rhs> $op_class<Rhs> for &'a HiSparseBitsetOp<Op, S1, S2> {
            type Output = HiSparseBitsetOp<$binary_op, &'a HiSparseBitsetOp<Op, S1, S2>, Rhs>;

            #[inline]
            fn $op_fn(self, rhs: Rhs) -> Self::Output {
                HiSparseBitsetOp::new($binary_op, self, rhs)
            }
        }

        impl<Op, S, Rhs> $op_class<Rhs> for Reduce<Op, S> {
            type Output = HiSparseBitsetOp<$binary_op, Reduce<Op, S>, Rhs>;

            #[inline]
            fn $op_fn(self, rhs: Rhs) -> Self::Output {
                HiSparseBitsetOp::new($binary_op, self, rhs)
            }
        }

        impl<'a, Op, S, Rhs> $op_class<Rhs> for &'a Reduce<Op, S> {
            type Output = HiSparseBitsetOp<$binary_op, &'a Reduce<Op, S>, Rhs>;

            #[inline]
            fn $op_fn(self, rhs: Rhs) -> Self::Output {
                HiSparseBitsetOp::new($binary_op, self, rhs)
            }
        }
    }
}

impl_op!(BitOr, bitor, BitOrOp);
impl_op!(BitAnd, bitand, BitAndOp);
impl_op!(BitXor, bitxor, BitXorOp);
impl_op!(Sub, sub, BitSubOp);

#[cfg(test)]
mod test{
    use std::collections::HashSet;
    use itertools::assert_equal;
    use rand::Rng;
    use rand::seq::IteratorRandom;
    use crate::reduce;
    use super::*;

    type HiSparseBitset = crate::HiSparseBitset<crate::configs::_64bit>;

    #[test]
    fn ops_test(){
        cfg_if::cfg_if! {
        if #[cfg(miri)] {
            const MAX_RANGE: usize = 10_000;
            const AMOUNT   : usize = 1;
            const INDEX_MUL: usize = 5;
        } else {
            const MAX_RANGE: usize = 10_000;
            const AMOUNT   : usize = 1000;
            const INDEX_MUL: usize = 5;
        }
        }

        let mut rng = rand::thread_rng();
        let mut v1 = Vec::new();
        let mut v2 = Vec::new();
        let mut v3 = Vec::new();
        let mut v4 = Vec::new();
        for _ in 0..AMOUNT{
            v1.push(rng.gen_range(0..MAX_RANGE)*INDEX_MUL);
            v2.push(rng.gen_range(0..MAX_RANGE)*INDEX_MUL);
            v3.push(rng.gen_range(0..MAX_RANGE)*INDEX_MUL);
            v4.push(rng.gen_range(0..MAX_RANGE)*INDEX_MUL);
        }

        /*
        // This is incredibly slow with MIRI
        let v1 = (0..MAX_RANGE).map(|i|i*INDEX_MUL).choose_multiple(&mut rng, AMOUNT);
        let v2 = (0..MAX_RANGE).map(|i|i*INDEX_MUL).choose_multiple(&mut rng, AMOUNT);
        let v3 = (0..MAX_RANGE).map(|i|i*INDEX_MUL).choose_multiple(&mut rng, AMOUNT);
        let v4 = (0..MAX_RANGE).map(|i|i*INDEX_MUL).choose_multiple(&mut rng, AMOUNT);
         */

        /*let hiset1: HiSparseBitset = v1.iter().copied().collect();
        let hiset2: HiSparseBitset = v2.iter().copied().collect();
        let hiset3: HiSparseBitset = v3.iter().copied().collect();
        let hiset4: HiSparseBitset = v4.iter().copied().collect();*/

        let set1: HashSet<usize> = v1.iter().copied().collect();
        let set2: HashSet<usize> = v2.iter().copied().collect();
        let set3: HashSet<usize> = v3.iter().copied().collect();
        let set4: HashSet<usize> = v4.iter().copied().collect();

        fn test<Op, S1, S2>(h: HiSparseBitsetOp<Op, S1, S2>, s: HashSet<usize>)
        where
            Op: BinaryOp,
            S1: LevelMasksExt3<Config = S2::Config>,
            S2: LevelMasksExt3,
        {
            let hv: Vec<usize> = h.iter_ext3()
                .flat_map(|block| block.iter())
                .collect();

            let mut s: Vec<usize> = s.into_iter().collect();
            s.sort();
            assert_equal(hv, s);
        }

        /*// &HiSet <-> &HiSet
        test(&hiset1 & &hiset2, &set1 & &set2);
        test(&hiset1 | &hiset2, &set1 | &set2);
        test(&hiset1 ^ &hiset2, &set1 ^ &set2);
        test(&hiset1 - &hiset2, &set1 - &set2);*/


        let hiset1 = HiSparseBitset::from([0]);
        let hiset2 = HiSparseBitset::from([0]);
        let hiset3 = HiSparseBitset::from([4096]);
        let hiset4 = HiSparseBitset::from([4096]);

        // Reduce <-> Reduce
        let group1 = [&hiset1, &hiset2];
        let group2 = [&hiset3, &hiset4];
        let reduce1 = reduce(BitOrOp, group1.iter().copied()).unwrap();
        let reduce2 = reduce(BitOrOp, group2.iter().copied()).unwrap();
        let set_or1 = &set1 | &set2;
        let set_or2 = &set3 | &set4;


        {
            let op = (reduce1.clone() | reduce2.clone());
            let hv: Vec<usize> = op.iter_ext3()
                .flat_map(|block| block.iter())
                .collect();

            //op.iter_ext3().next();
        }


        return;
        /*test(
            reduce1.clone() & reduce2.clone(),
            &set_or1        & &set_or2
        );*/
        test(
            reduce1.clone() | reduce2.clone(),
            &set_or1        | &set_or2
        );
                return;

        test(
            reduce1.clone() ^ reduce2.clone(),
            &set_or1        ^ &set_or2
        );
        test(
            reduce1.clone() - reduce2.clone(),
            &set_or1        - &set_or2
        );

        // &Reduce <-> &Reduce
        test(
            &reduce1 & &reduce2,
            &set_or1 & &set_or2
        );
        test(
            &reduce1 | &reduce2,
            &set_or1 | &set_or2
        );
        test(
            &reduce1 ^ &reduce2,
            &set_or1 ^ &set_or2
        );
        test(
            &reduce1 - &reduce2,
            &set_or1 - &set_or2
        );

        // Op <-> Op
        let hiset_or1 = &hiset1 | &hiset2;
        let hiset_or2 = &hiset3 | &hiset4;
        test(hiset_or1.clone() & hiset_or2.clone(), &set_or1 & &set_or2);
        test(hiset_or1.clone() | hiset_or2.clone(), &set_or1 | &set_or2);
        test(hiset_or1.clone() ^ hiset_or2.clone(), &set_or1 ^ &set_or2);
        test(hiset_or1.clone() - hiset_or2.clone(), &set_or1 - &set_or2);

        // &Op <-> &Op
        test(&hiset_or1 & &hiset_or2, &set_or1 & &set_or2);
        test(&hiset_or1 | &hiset_or2, &set_or1 | &set_or2);
        test(&hiset_or1 ^ &hiset_or2, &set_or1 ^ &set_or2);
        test(&hiset_or1 - &hiset_or2, &set_or1 - &set_or2);
    }
}