use std::marker::PhantomData;
use std::mem;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ptr::addr_of_mut;
use crate::ops::*;
use crate::BitSetInterface;
use crate::implement::impl_bitset;
use crate::bitset_interface::{BitSetBase, LevelMasks, LevelMasksIterExt};
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

impl<Op, S1, S2> LevelMasksIterExt for Apply<Op, S1, S2>
where
    Op: BitSetOp,
    S1: LevelMasksIterExt,
    S2: LevelMasksIterExt<Conf = S1::Conf>,
{
    type Level1BlockData = (S1::Level1BlockData, S2::Level1BlockData);

    type IterState = (S1::IterState, S2::IterState);

    #[inline]
    fn make_iter_state(&self) -> Self::IterState {
        (self.s1.make_iter_state(), self.s2.make_iter_state())
    }

    #[inline]
    unsafe fn drop_iter_state(&self, state: &mut ManuallyDrop<Self::IterState>) {
        unsafe{
            self.s1.drop_iter_state(mem::transmute(&mut state.0));
            self.s2.drop_iter_state(mem::transmute(&mut state.1));
        }
    }

    #[inline]
    unsafe fn init_level1_block_data(
        &self,
        state: &mut Self::IterState,
        level1_block_data: &mut MaybeUninit<Self::Level1BlockData>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        // &mut MaybeUninit<(T0, T1)> = (&mut MaybeUninit<T0>, &mut MaybeUninit<T1>) 
        let (level1_block_data0, level1_block_data1) = {
            let ptr = level1_block_data.as_mut_ptr();
            let ptr0 = addr_of_mut!((*ptr).0);
            let ptr1 = addr_of_mut!((*ptr).1);
            (
                &mut*mem::transmute::<_, *mut MaybeUninit<S1::Level1BlockData>>(ptr0), 
                &mut*mem::transmute::<_, *mut MaybeUninit<S2::Level1BlockData>>(ptr1)
            )
        };
        
        let (mask1, v1) = self.s1.init_level1_block_data(
            &mut state.0, level1_block_data0, level0_index
        );
        let (mask2, v2) = self.s2.init_level1_block_data(
            &mut state.1, level1_block_data1, level0_index
        );

        let mask = Op::hierarchy_op(mask1, mask2);
        (mask, v1 | v2)
    }

    #[inline]
    unsafe fn data_mask_from_block_data(
        level1_blocks: &Self::Level1BlockData, level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock {
        let m0 = S1::data_mask_from_block_data(
            &level1_blocks.0, level1_index
        );
        let m1 = S2::data_mask_from_block_data(
            &level1_blocks.1, level1_index
        ); 
        Op::data_op(m0, m1)
    }
}

impl_bitset!(
    impl<Op, S1, S2> for Apply<Op, S1, S2> 
    where 
        Op: BitSetOp, 
        S1: BitSetInterface, 
        S2: BitSetInterface<Conf = S1::Conf>
);

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
            S1: BitSetInterface<Conf = S2::Conf>,
            S2: BitSetInterface,
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