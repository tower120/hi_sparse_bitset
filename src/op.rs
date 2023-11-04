use std::marker::PhantomData;
use std::ops::{BitOr, BitAnd, BitXor, Sub};
use crate::binary_op::*;
use crate::{HiSparseBitset, IConfig};
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
    type Level1Blocks3 = (S1::Level1Blocks3, S2::Level1Blocks3);

    #[inline]
    fn make_level1_blocks3(&self) -> Self::Level1Blocks3 {
        (self.s1.make_level1_blocks3(), self.s2.make_level1_blocks3())
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        &self, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        let (mask1, v1) = self.s1.update_level1_blocks3(&mut level1_blocks.0, level0_index);
        let (mask2, v2) = self.s2.update_level1_blocks3(&mut level1_blocks.1, level0_index);
        let mask = Op::hierarchy_op(mask1, mask2);
        (mask, v1 | v2)
    }

    #[inline]
    unsafe fn data_mask_from_blocks3(
        level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock {
        let m0 = S1::data_mask_from_blocks3(&level1_blocks.0, level1_index);
        let m1 = S2::data_mask_from_blocks3(&level1_blocks.1, level1_index);
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
    use rand::seq::IteratorRandom;
    use crate::reduce;
    use super::*;

    type HiSparseBitset = crate::HiSparseBitset<crate::configs::_64bit>;

    #[test]
    fn ops_test(){
        let mut rng = rand::thread_rng();
        let v1 = (0..10_000).choose_multiple(&mut rng, 1000);
        let v2 = (0..10_000).choose_multiple(&mut rng, 1000);
        let v3 = (0..10_000).choose_multiple(&mut rng, 1000);
        let v4 = (0..10_000).choose_multiple(&mut rng, 1000);
        let hiset1: HiSparseBitset = v1.iter().copied().collect();
        let hiset2: HiSparseBitset = v2.iter().copied().collect();
        let hiset3: HiSparseBitset = v3.iter().copied().collect();
        let hiset4: HiSparseBitset = v4.iter().copied().collect();

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

        // &HiSet <-> &HiSet
        test(&hiset1 & &hiset2, &set1 & &set2);
        test(&hiset1 | &hiset2, &set1 | &set2);
        test(&hiset1 ^ &hiset2, &set1 ^ &set2);
        test(&hiset1 - &hiset2, &set1 - &set2);

        // Reduce <-> Reduce
        let reduce1 = reduce(BitOrOp, [&hiset1, &hiset2].into_iter()).unwrap();
        let reduce2 = reduce(BitOrOp, [&hiset3, &hiset4].into_iter()).unwrap();
        let set_or1 = &set1 | &set2;
        let set_or2 = &set3 | &set4;
        test(
            reduce1.clone() & reduce2.clone(),
            &set_or1        & &set_or2
        );
        test(
            reduce1.clone() | reduce2.clone(),
            &set_or1        | &set_or2
        );
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