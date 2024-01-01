use std::any::TypeId;
use std::marker::PhantomData;
use std::mem;
use std::mem::{ManuallyDrop, MaybeUninit};
//use std::ops::{BitAnd, BitOr, BitXor, Sub};
use crate::ops::*;
use crate::BitSet;
use crate::bit_block::BitBlock;
use crate::reduce::Reduce;
use crate::bitset_interface::{BitSetBase, /*duplicate_bitset_interface,*/ LevelMasks, LevelMasksExt};
use crate::config::Config;

/// Binary operation application, as lazy bitset.
///
/// Created by [apply], or by applying [BitOr], [BitAnd], [BitXor],
/// [Sub] operations on [BitSetInterface]s.
/// 
/// [BitOr]: std::ops::BitOr
/// [BitAnd]: std::ops::BitAnd 
/// [BitXor]: std::ops::BitXor
/// [Sub]: std::ops::Sub
/// [apply]: crate::apply()
/// [BitSetInterface]: crate::BitSetInterface
#[derive(Clone)]
pub struct Apply<Op, S1, S2>{
    pub(crate) s1: S1,
    pub(crate) s2: S2,
    pub(crate) phantom: PhantomData<Op>
}
impl<Op, S1, S2> Apply<Op, S1, S2>{
    #[inline]
    pub(crate) fn new(_:Op, s1:S1, s2:S2) -> Self{
        Apply { s1, s2, phantom:PhantomData }
    }
}

impl<Op, S1, S2> BitSetBase for Apply<Op, S1, S2>
where
    Op: BitSetOp,
    S1: LevelMasks,
    S2: LevelMasks<Conf = S1::Conf>,
{
    type Conf = S1::Conf;
    
    /// true if S1, S2 and Op are `TrustedHierarchy`. 
    const TRUSTED_HIERARCHY: bool = 
        Op::TRUSTED_HIERARCHY 
        & S1::TRUSTED_HIERARCHY & S2::TRUSTED_HIERARCHY;
}

impl<Op, S1, S2> LevelMasks for Apply<Op, S1, S2>
where
    Op: BitSetOp,
    S1: LevelMasks,
    S2: LevelMasks<Conf= S1::Conf>,
{
    #[inline]
    fn level0_mask(&self) -> <Self::Conf as Config>::Level0BitBlock {
        Op::hierarchy_op(self.s1.level0_mask(), self.s2.level0_mask())
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize)
        -> <Self::Conf as Config>::Level1BitBlock
    {
        Op::hierarchy_op(
            self.s1.level1_mask(level0_index),
            self.s2.level1_mask(level0_index)
        )
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize)
        -> <Self::Conf as Config>::DataBitBlock
    {
        Op::data_op(
            self.s1.data_mask(level0_index, level1_index),
            self.s2.data_mask(level0_index, level1_index)
        )
    }
}

impl<Op, S1, S2> LevelMasksExt for Apply<Op, S1, S2>
where
    Op: BitSetOp,
    S1: LevelMasksExt,
    S2: LevelMasksExt<Conf = S1::Conf>,
{
    type Level1Blocks = (MaybeUninit<S1::Level1Blocks>, MaybeUninit<S2::Level1Blocks>, MaybeUninit<bool>, MaybeUninit<bool>);

    const EMPTY_LVL1_TOLERANCE: bool = true;

    type CacheData = (S1::CacheData, S2::CacheData);

    #[inline]
    fn make_cache(&self) -> Self::CacheData {
        (self.s1.make_cache(), self.s2.make_cache())
    }

    #[inline]
    fn drop_cache(&self, cache: &mut ManuallyDrop<Self::CacheData>) {
        unsafe{
            self.s1.drop_cache(mem::transmute(&mut cache.0));
            self.s2.drop_cache(mem::transmute(&mut cache.1));
        }
    }

    #[inline]
    unsafe fn update_level1_blocks(
        &self,
        cache_data: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        let level1_blocks = level1_blocks.assume_init_mut();
        let (mask1, v1) = self.s1.update_level1_blocks(
            &mut cache_data.0, &mut level1_blocks.0, level0_index
        );
        let (mask2, v2) = self.s2.update_level1_blocks(
            &mut cache_data.1, &mut level1_blocks.1, level0_index
        );

        /*const*/ let is_intersection = TypeId::of::<Op>() == TypeId::of::<And>();
        if !is_intersection {
        if !S1::EMPTY_LVL1_TOLERANCE {
            level1_blocks.2.write(v1);
        }
        if !S2::EMPTY_LVL1_TOLERANCE {
            level1_blocks.3.write(v2);
        }
        }

        let mask = Op::hierarchy_op(mask1, mask2);
        (mask, v1 | v2)
    }

    #[inline]
    unsafe fn data_mask_from_blocks(
        level1_blocks: &Self::Level1Blocks, level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock {
        // intersection can never point to empty blocks.
        /*const*/ let is_intersection = TypeId::of::<Op>() == TypeId::of::<And>();

        let m0 = if S1::EMPTY_LVL1_TOLERANCE || is_intersection || level1_blocks.2.assume_init(){
            S1::data_mask_from_blocks(level1_blocks.0.assume_init_ref(), level1_index)
        } else {
            <Self::Conf as Config>::DataBitBlock::zero()
        };

        let m1 = if S2::EMPTY_LVL1_TOLERANCE || is_intersection || level1_blocks.3.assume_init(){
            S2::data_mask_from_blocks(level1_blocks.1.assume_init_ref(), level1_index)
        } else {
            <Self::Conf as Config>::DataBitBlock::zero()
        };

        Op::data_op(m0, m1)
    }
}


// We need this all because RUST still does not support template/generic specialization.
macro_rules! impl_op {
    (impl <$($generics:tt),*> for $t:ty where $($where_bounds:tt)*) => {

        impl<$($generics),*, Rhs> std::ops::BitAnd<Rhs> for $t
        where
            $($where_bounds)*
        {
            type Output = Apply<And, $t, Rhs>;

            /// Returns intersection of self and rhs bitsets.
            #[inline]
            fn bitand(self, rhs: Rhs) -> Self::Output{
                Apply::new(And, self, rhs)    
            }
        }

        impl<$($generics),*, Rhs> std::ops::BitOr<Rhs> for $t
        where
            $($where_bounds)*
        {
            type Output = Apply<Or, $t, Rhs>;

            /// Returns union of self and rhs bitsets.
            #[inline]
            fn bitor(self, rhs: Rhs) -> Self::Output{
                Apply::new(Or, self, rhs)    
            }
        }

        impl<$($generics),*, Rhs> std::ops::BitXor<Rhs> for $t
        where
            $($where_bounds)*
        {
            type Output = Apply<Xor, $t, Rhs>;

            /// Returns symmetric difference of self and rhs bitsets.
            #[inline]
            fn bitxor(self, rhs: Rhs) -> Self::Output{
                Apply::new(Xor, self, rhs)    
            }
        }        

        impl<$($generics),*, Rhs> std::ops::Sub<Rhs> for $t
        where
            $($where_bounds)*
        {
            type Output = Apply<Sub, $t, Rhs>;

            /// Returns difference of self and rhs bitsets. 
            ///
            /// _Or relative complement of rhs in self._
            #[inline]
            fn sub(self, rhs: Rhs) -> Self::Output{
                Apply::new(Sub, self, rhs)    
            }
        }    

    };
}

impl_op!(impl<Conf> for BitSet<Conf> where Conf: Config);
impl_op!(impl<'a, Conf> for &'a BitSet<Conf> where Conf: Config);
impl_op!(impl<Op, S1, S2> for Apply<Op, S1, S2> where /* S1: BitSetInterface, S2: BitSetInterface */);
impl_op!(impl<'a, Op, S1, S2> for &'a Apply<Op, S1, S2> where /* S1: BitSetInterface, S2: BitSetInterface */);
impl_op!(impl<Op, S, Storage> for Reduce<Op, S, Storage> where);
impl_op!(impl<'a, Op, S, Storage> for &'a Reduce<Op, S, Storage> where);

#[cfg(test)]
mod test{
    use std::collections::HashSet;
    use itertools::assert_equal;
    use rand::Rng;
    use crate::reduce;
    use super::*;

    type HiSparseBitset = crate::BitSet<crate::config::_64bit>;

    #[test]
    fn ops_test(){
        cfg_if::cfg_if! {
        if #[cfg(miri)] {
            const MAX_RANGE: usize = 10_000;
            const AMOUNT   : usize = 100;
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

        let hiset1: HiSparseBitset = v1.iter().copied().collect();
        let hiset2: HiSparseBitset = v2.iter().copied().collect();
        let hiset3: HiSparseBitset = v3.iter().copied().collect();
        let hiset4: HiSparseBitset = v4.iter().copied().collect();

        let set1: HashSet<usize> = v1.iter().copied().collect();
        let set2: HashSet<usize> = v2.iter().copied().collect();
        let set3: HashSet<usize> = v3.iter().copied().collect();
        let set4: HashSet<usize> = v4.iter().copied().collect();

        fn test<Op, S1, S2>(h: Apply<Op, S1, S2>, s: HashSet<usize>)
        where
            Op: BitSetOp,
            S1: LevelMasksExt<Conf = S2::Conf>,
            S2: LevelMasksExt,
        {
            let hv: Vec<usize> = h.block_iter()
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
        let group1 = [&hiset1, &hiset2];
        let group2 = [&hiset3, &hiset4];
        let reduce1 = reduce(Or, group1.iter().copied()).unwrap();
        let reduce2 = reduce(Or, group2.iter().copied()).unwrap();
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