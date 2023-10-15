//pub mod intersection_blocks_resumable;
mod block;
mod level;
mod bitblock;
mod bit_op;

use std::{ops::ControlFlow};
use num_traits::{AsPrimitive, PrimInt};
//use crate::utils::{simd_op::{SimdOp, SimdVec128}, bit_op, primitive_traits::{Primitive, AsPrimitive}};

use block::Block;
use level::Level;
use crate::bitblock::BitBlock;
/*use crate::block::IBlock;
use crate::level::ILevel;
*/
/// 0 level mask should have size <= 256
/*type Level0Mask = SimdVec128;
type Level1Mask = SimdVec128;
pub type DataBlock  = SimdVec128;

type Level1BlockIndex = u8;
type DataBlockIndex   = u16;

type Level0BlockIndices = [Level1BlockIndex; 1<< Level0Mask::SIZE_POT_EXPONENT];
type Level1BlockIndices = [DataBlockIndex  ; 1<< Level1Mask::SIZE_POT_EXPONENT];
type NoBlockIndices     = [usize;0];

type Level0Block    = Block<Level0Mask, Level1BlockIndex, Level0BlockIndices>;
type Level1Block    = Block<Level1Mask, DataBlockIndex,   Level1BlockIndices>;
type LevelDataBlock = Block<DataBlock,  usize,            NoBlockIndices>;

type Level0    = Level0Block;
type Level1    = Level<Level1Block, Level1BlockIndex>;
type LevelData = Level<LevelDataBlock, DataBlockIndex>;*/

pub trait MyPrimitive: PrimInt + AsPrimitive<usize> + Default + 'static
/*where
    Self: 'static,
    usize: AsPrimitive<Self>*/
{}

pub trait IConfig: Default {
    type Level0BitBlock: BitBlock + Default;
    type Level0BlockIndices: AsRef<[Self::Level1BlockIndex]> + AsMut<[Self::Level1BlockIndex]> + Default;

    type Level1BitBlock: BitBlock + Default;
    type Level1BlockIndex: MyPrimitive;
    type Level1BlockIndices: AsRef<[Self::DataBlockIndex]> + AsMut<[Self::DataBlockIndex]> + Default;

    type DataBitBlock: BitBlock + Default;
    type DataBlockIndex: MyPrimitive;

/*    type Level0    : IBlock + Default;
    type Level1    : ILevel + Default;
    type LevelData : ILevel + Default;*/
}

/// Hierarchical sparse bitset. Tri-level hierarchy. Highest uint it can hold is Level0Mask * Level1Mask * DenseBlock.
/// 
/// Only last level contains blocks of actual data. Empty(skipped) data blocks are not allocated.
/// 
/// Structure optimized for intersection speed. Insert/remove/contains is fast O(1) too.
#[derive(Default, Clone)]
pub struct HiSparseBitset<Config: IConfig>{
    level0: Block<Config::Level0BitBlock, Config::Level1BlockIndex, Config::Level0BlockIndices>,
    level1: Level<
                Block<Config::Level1BitBlock, Config::DataBlockIndex, Config::Level1BlockIndices>,
                Config::Level1BlockIndex,
            >,
    data  : Level<
                Block<Config::DataBitBlock, usize, [usize;0]>,
                Config::DataBlockIndex,
            >,
/*    level0: Config::Level0,
    level1: Config::Level1,
    data  : Config::LevelData*/
}

impl<Config: IConfig> HiSparseBitset<Config>
where
    //Config::Level1BlockIndex: AsPrimitive<usize>,
    usize: AsPrimitive<Config::Level1BlockIndex>,

    //Config::DataBlockIndex: AsPrimitive<usize>,
    usize: AsPrimitive<Config::DataBlockIndex>,
{
    #[inline]
    pub fn new() -> Self{
        Self::default()
    }

    #[inline]
    fn level_indices(index: usize) -> (usize/*level0*/, usize/*level1*/, usize/*data*/){
        // this should be const and act as const.
        let DATA_BLOCK_SIZE_POT_EXP  : usize = <<Config as IConfig>::DataBitBlock as BitBlock>::SIZE_POT_EXPONENT;
        //let DATA_BLOCK_SIZE_POT_EXP  : usize = <<<Config as IConfig>::LevelData as ILevel>::Block as IBlock>::SIZE_POT_EXPONENT;
        //let DATA_BLOCK_SIZE_POT_EXP  : usize = <<<Config as IConfig>::LevelData as ILevel>::Block as IBlock>::SIZE_POT_EXPONENT;
        //let LEVEL1_BLOCK_SIZE_POT_EXP: usize = <<<Config as IConfig>::Level1    as ILevel>::Block as IBlock>::SIZE_POT_EXPONENT;
        let LEVEL1_BLOCK_SIZE_POT_EXP: usize = <<Config as IConfig>::Level1BitBlock as BitBlock>::SIZE_POT_EXPONENT;

        // const DATA_BLOCK_SIZE:  usize = 1 << DenseBlock::SIZE_POT_EXPONENT;
        let DATA_BLOCK_CAPACITY_POT_EXP:  usize = DATA_BLOCK_SIZE_POT_EXP;
        // const LEVEL1_BLOCK_SIZE: usize = (1 << Level1Mask::SIZE_POT_EXPONENT) * DATA_BLOCK_SIZE;
        let LEVEL1_BLOCK_CAPACITY_POT_EXP: usize = LEVEL1_BLOCK_SIZE_POT_EXP + DATA_BLOCK_SIZE_POT_EXP;

        // index / LEVEL1_BLOCK_SIZE
        let level0 = index >> LEVEL1_BLOCK_CAPACITY_POT_EXP;
        // index - (level0 * LEVEL1_BLOCK_SIZE)
        let level0_remainder = index - (level0 << LEVEL1_BLOCK_CAPACITY_POT_EXP);

        // level0_remainder / DATA_BLOCK_SIZE
        let level1 = level0_remainder >> DATA_BLOCK_CAPACITY_POT_EXP;
        // level0_remainder - (level1 * DATA_BLOCK_SIZE)
        let level1_remainder = level0_remainder - (level1 << DATA_BLOCK_CAPACITY_POT_EXP);

        let data = level1_remainder;

        (level0, level1, data)
    }

    #[inline]
    fn get_block_indices(&self, level0_index: usize, level1_index: usize)
        -> Option<(Config::Level1BlockIndex, Config::DataBlockIndex)>
    {
        // 1. Level0
        let level1_block_index = unsafe{
            self.level0.get(level0_index)?
        };

        // 2. Level1
        let data_block_index = unsafe{
            let level1_block = self.level1.blocks().get_unchecked(level1_block_index.as_());
            level1_block.get(level1_index)?
        };

        Some((level1_block_index, data_block_index))
    }

    pub fn insert(&mut self, index: usize){
        // That's indices to next level
        let (level0_index, level1_index, data_index) = Self::level_indices(index);

        // 1. Level0
        let level1_block_index = unsafe{
            self.level0.get_or_insert(level0_index, ||self.level1.insert_block())
        }.as_();

        // 2. Level1
        let data_block_index = unsafe{
            let level1_block = self.level1.blocks_mut().get_unchecked_mut(level1_block_index);
            level1_block.get_or_insert(level1_index, ||self.data.insert_block())
        }.as_();

        // 3. Data level
        unsafe{
            let data_block = self.data.blocks_mut().get_unchecked_mut(data_block_index);
            data_block.insert_mask_unchecked(data_index);
        }
    }

    /// Returns false if index is invalid/was not in bitset
    pub fn remove(&mut self, index: usize) -> bool {
        // 1. Resolve indices
        let (level0_index, level1_index, data_index) = Self::level_indices(index);
        let (level1_block_index, data_block_index) = match self.get_block_indices(level0_index, level1_index){
            None => return false,
            Some(value) => value,
        };

        unsafe{
            // 2. Get Data block and set bit
            let data_block = self.data.blocks_mut().get_unchecked_mut(data_block_index.as_());
            let existed = data_block.remove(data_index);

            if existed{
                // 3. Remove free blocks
                if data_block.is_empty(){
                    // remove data block
                    self.data.remove_empty_block_unchecked(data_block_index);

                    // remove pointer from level1
                    let level1_block = self.level1.blocks_mut().get_unchecked_mut(level1_block_index.as_());
                    level1_block.remove(level1_index);

                    if level1_block.is_empty(){
                        // remove level1 block
                        self.level1.remove_empty_block_unchecked(level1_block_index);

                        // remove pointer from level0
                        self.level0.remove(level0_index);
                    }
                }
            }
            existed
        }
    }

    /// # Safety
    ///
    /// index MUST exists in HiSparseBitset!
    #[inline]
    pub unsafe fn remove_unchecked(&mut self, index: usize) {
        // TODO: make sure compiler actually get rid of unused code.
        let ok = self.remove(index);
        if !ok {
            unsafe{ std::hint::unreachable_unchecked(); }
        }
    }

    pub fn contains(&self, index: usize) -> bool {
        let (level0_index, level1_index, data_index) = Self::level_indices(index);
        let (level1_block_index, data_block_index) = match self.get_block_indices(level0_index, level1_index){
            None => return false,
            Some(value) => value,
        };

        // 3. Data level
        unsafe{
            let data_block = self.data.blocks().get_unchecked(data_block_index.as_());
            data_block.contains(data_index)
        }
    }
}

/*impl FromIterator<usize> for HiSparseBitset{
    fn from_iter<T: IntoIterator<Item=usize>>(iter: T) -> Self {
        let mut this = Self::default();
        for i in iter{
            this.insert(i);
        }
        this
    }
}

// TODO: Consider using &IntoIterator instead of cloning iterator?
// See doc/HiSparseBitset.png for illustration.
//
// On each level We first calculate intersection mask between all sets, 
// then depth traverse only intersected elements/indices/blocks.
/// `sets` iterator will be cloned multiple times.
pub fn intersection_blocks_traverse<'a, S, F>(sets: S, mut foreach_block: F)
where
    S: IntoIterator<Item = &'a HiSparseBitset>,
    S::IntoIter: Clone,
    F: FnMut(usize/*block_start_index*/, DataBlock)
{
    use ControlFlow::*;
    let sets = sets.into_iter();

    // Level0
    let level0_intersection = 
        sets.clone()
        .map(|set| *set.level0.mask())
        .reduce(SimdOp::and);

    let level0_intersection = match level0_intersection{
        Some(intersection) => intersection,
        None => return,
    };
    if SimdOp::is_zero(level0_intersection){
        return;
    }
    
    SimdOp::traverse_one_indices(
        level0_intersection, 
        |level0_index| level1_intersection_traverse(sets.clone(), level0_index, &mut foreach_block)
    );

    // Level1
    #[inline]
    fn level1_intersection_traverse<'a>(
        sets: impl Iterator<Item = &'a HiSparseBitset> + Clone,
        level0_index: usize, 
        foreach_block: &mut impl FnMut(usize/*block_start_index*/, DataBlock)
    ) -> ControlFlow<()> {
        let level1_intersection = unsafe{
            sets.clone()
            .map(|set| {
                let level1_block_index = set.level0.get_unchecked(level0_index);
                let level1_block = set.level1.blocks().get_unchecked(level1_block_index as usize);
                *level1_block.mask()
            })
            .reduce(SimdOp::and)
            .unwrap_unchecked()
        };

        SimdOp::traverse_one_indices(
            level1_intersection, 
            |level1_index| data_intersection_traverse(sets.clone(), level0_index, level1_index, foreach_block)
        );

        Continue(())
    }

    // Data
    #[inline]
    fn data_intersection_traverse<'a>(
        sets: impl Iterator<Item = &'a HiSparseBitset>,
        level0_index: usize, 
        level1_index: usize,
        foreach_block: &mut impl FnMut(usize/*block_start_index*/, DataBlock)
    ) -> ControlFlow<()>{
        let data_intersection = unsafe{
            sets
            .map(|set| {
                // We could collect level1_block_index/&level1_block during level1 walk,
                // but benchmarks showed that does not have measurable performance benefits.

                let level1_block_index = set.level0.get_unchecked(level0_index);
                let level1_block       = set.level1.blocks().get_unchecked(level1_block_index as usize);

                let data_block_index   = level1_block.get_unchecked(level1_index);
                *set.data.blocks().get_unchecked(data_block_index as usize).mask()
            })
            .reduce(SimdOp::and)
            .unwrap_unchecked()
        };

        let block_start_index = (level0_index << (DataBlock::SIZE_POT_EXPONENT + Level1Mask::SIZE_POT_EXPONENT))
                              + (level1_index << (DataBlock::SIZE_POT_EXPONENT));

        (foreach_block)(block_start_index, data_intersection);

        Continue(())
    }
}

/// Same as [intersection_blocks_traverse], but iterator, and a tiny bit slower.
/// 
/// `sets` iterator will be cloned and iterated multiple times.
#[inline]
pub fn intersection_blocks<'a, S>(sets: S)
    -> intersection_blocks_resumable::IntersectionBlocks<'a, S::IntoIter>
where
    S: IntoIterator<Item = &'a HiSparseBitset>,
    S::IntoIter: Clone,
{
    intersection_blocks_resumable::IntersectionBlocks::new(sets.into_iter())
}

/// For Debug purposes.
pub fn collect_intersection(sets: &[HiSparseBitset]) -> Vec<usize>{
    use ControlFlow::*;
    let mut indices = Vec::new();
    intersection_blocks_traverse(sets, 
        |start_index, block|{
            SimdOp::traverse_one_indices(block, 
                |index|{
                    indices.push(start_index+index);
                    Continue(())
                }
            );
        }
    );
    indices
}
*/

/*#[cfg(test)]
mod test{
    use std::{collections::HashSet, hash::Hash};
    use std::iter::zip;

    use itertools::assert_equal;
    use rand::Rng;

    use crate::archetype::hi_spares_bitset::intersection_blocks_resumable::{IntersectionBlocksState, IntersectionBlocks};

    use super::*;

    #[test]
    fn level_indices_test(){
        // assuming all levels with 128bit blocks
        let levels = HiSparseBitset::level_indices(0);
        assert_eq!(levels, (0,0,0));

        let levels = HiSparseBitset::level_indices(10);
        assert_eq!(levels, (0,0,10));

        let levels = HiSparseBitset::level_indices(128);
        assert_eq!(levels, (0,1,0));

        let levels = HiSparseBitset::level_indices(130);
        assert_eq!(levels, (0,1,2));

        let levels = HiSparseBitset::level_indices(130);
        assert_eq!(levels, (0,1,2));

        let levels = HiSparseBitset::level_indices(128*128);
        assert_eq!(levels, (1,0,0));

        let levels = HiSparseBitset::level_indices(128*128 + 50*128);
        assert_eq!(levels, (1,50,0));

        let levels = HiSparseBitset::level_indices(128*128 + 50*128 + 4);
        assert_eq!(levels, (1,50,4));
    }

    #[test]
    fn smoke_test(){
        let mut set = HiSparseBitset::default();

        assert!(!set.contains(0));
        set.insert(0);
        assert!(set.contains(0));
    }

    #[test]
    fn fuzzy_test(){
        const MAX_SIZE : usize = 10000;
        const MAX_RANGE: usize = 1000000;
        const CONTAINS_PROBES: usize = 1000;

        let mut rng = rand::thread_rng();
        for _ in 0..100{
            let mut hash_set = HashSet::new();
            let mut hi_set = HiSparseBitset::default();

            let mut inserted = Vec::new();
            let mut removed = Vec::new();

            for _ in 0..10{
                // random insert
                for _ in 0..rng.gen_range(0..MAX_SIZE){
                    let index = rng.gen_range(0..MAX_RANGE);
                    inserted.push(index);
                    hash_set.insert(index);
                    hi_set.insert(index);
                }

                // random remove
                if !inserted.is_empty(){
                for _ in 0..rng.gen_range(0..inserted.len()){
                    let index = rng.gen_range(0..inserted.len());
                    let value = inserted[index];
                    removed.push(value);
                    hash_set.remove(&value);
                    hi_set.remove(value);
                }
                }

                // random contains
                for _ in 0..CONTAINS_PROBES{
                    let index = rng.gen_range(0..MAX_RANGE);
                    let h1 = hash_set.contains(&index);
                    let h2 = hi_set.contains(index);
                    assert_eq!(h1, h2);
                }

                // existent contains
                for &index in &hash_set{
                    assert!(hi_set.contains(index));
                }

                // non existent does not contains
                for &index in &removed{
                    let h1 = hash_set.contains(&index);
                    let h2 = hi_set.contains(index);
                    assert_eq!(h1, h2);
                }
            }
        }
    }

    #[test]
    fn fuzzy_intersection_test(){
        const MAX_SETS : usize = 10;
        const MAX_INSERTS: usize = 10000;
        const MAX_GUARANTEED_INTERSECTIONS: usize = 10;
        const MAX_REMOVES : usize = 10000;
        const MAX_RANGE: usize = 1000000;
        const MAX_RESUMED_INTERSECTION_BLOCKS_CONSUME: usize = 100;

        fn hashset_multi_intersection<'a, T: Eq + Hash + Copy + 'a>(hash_sets: impl IntoIterator<Item = &'a HashSet<T>>) -> HashSet<T>
        {
            let mut hash_sets_iter = hash_sets.into_iter();
            let mut acc = hash_sets_iter.next().unwrap().clone();
            for set in hash_sets_iter{
                let intersection = acc.intersection(set)
                    .copied()
                    .collect();
                acc = intersection;
            }
            acc
        }

        let mut rng = rand::thread_rng();
        for _ in 0..100{
            let sets_count = rng.gen_range(2..MAX_SETS);
            let mut hash_sets: Vec<HashSet<usize>> = vec![Default::default(); sets_count];
            let mut hi_sets  : Vec<HiSparseBitset> = vec![Default::default(); sets_count];

            // Resumable intersection guarantee that we'll traverse at least
            // non removed initial intersection set.

            // initial insert
            let mut intersection_state = IntersectionBlocksState::default();
            let mut initial_hashsets_intersection;
            {
                for (hash_set, hi_set) in zip(hash_sets.iter_mut(), hi_sets.iter_mut()){
                    for _ in 0..rng.gen_range(0..MAX_INSERTS){
                        let index = rng.gen_range(0..MAX_RANGE);
                        hash_set.insert(index);
                        hi_set.insert(index);
                    }
                }
                initial_hashsets_intersection = hashset_multi_intersection(&hash_sets);
            }

            for _ in 0..10{
                // random insert
                for (hash_set, hi_set) in zip(hash_sets.iter_mut(), hi_sets.iter_mut()){
                    for _ in 0..rng.gen_range(0..MAX_INSERTS){
                        let index = rng.gen_range(0..MAX_RANGE);
                        hash_set.insert(index);
                        hi_set.insert(index);
                    }
                }

                // guaranteed intersection (insert all)
                for _ in 0..rng.gen_range(0..MAX_GUARANTEED_INTERSECTIONS){
                    let index = rng.gen_range(0..MAX_RANGE);
                    for hash_set in &mut hash_sets{
                        hash_set.insert(index);
                    }
                    for hi_set in &mut hi_sets{
                        hi_set.insert(index);
                    }
                }

                // random remove
                let mut removed = Vec::new();
                for (hash_set, hi_set) in zip(hash_sets.iter_mut(), hi_sets.iter_mut()){
                    for _ in 0..rng.gen_range(0..MAX_REMOVES){
                        let index = rng.gen_range(0..MAX_RANGE);
                        hash_set.remove(&index);
                        hi_set.remove(index);
                        removed.push(index);
                    }
                }

                // etalon intersection
                let hashsets_intersection = hashset_multi_intersection(&hash_sets);

                // remove non-existent intersections from initial_hashsets_intersection
                for index in &removed{
                    if !hashsets_intersection.contains(index){
                        initial_hashsets_intersection.remove(index);
                    }
                }

                // intersection resume
                {
                    let mut intersection = intersection_state.resume(hi_sets.iter());
                    let mut blocks_to_consume = rng.gen_range(0..MAX_RESUMED_INTERSECTION_BLOCKS_CONSUME);

                    // all intersections must be valid
                    loop{
                        if blocks_to_consume == 0{
                            break;
                        }
                        blocks_to_consume -= 1;

                        if let Some((start_index, block)) = intersection.next(){
                            SimdOp::traverse_one_indices(block,
                                |index|{
                                    let index = start_index + index;
                                    assert!(hashsets_intersection.contains(&index));
                                    initial_hashsets_intersection.remove(&index);
                                    ControlFlow::Continue(())
                                }
                            );
                        } else {
                            break;
                        }
                    }

                    intersection_state = intersection.suspend();
                }

                // intersection
                {
                    let mut hi_intersection = collect_intersection(&hi_sets);

                    // check that intersection_blocks = intersection_blocks_traverse
                    {
                        let mut indices2 = Vec::new();
                        for (start_index, block) in intersection_blocks(&hi_sets){
                            SimdOp::traverse_one_indices(block,
                                |index|{
                                    indices2.push(start_index+index);
                                    ControlFlow::Continue(())
                                }
                            );
                        }
                        assert_eq!(hi_intersection, indices2);
                    }

                    {
                        let mut indices2 = Vec::new();
                        let state = IntersectionBlocksState::default();
                        for (start_index, block) in state.resume(hi_sets.iter()){
                            SimdOp::traverse_one_indices(block,
                                |index|{
                                    indices2.push(start_index+index);
                                    ControlFlow::Continue(())
                                }
                            );
                        }

                        if hi_intersection != indices2{
                            println!("{:?}", hash_sets);
                            panic!();
                        }
                        //assert_eq!(hi_intersection, indices2);
                    }

                    let mut hashsets_intersection: Vec<usize> = hashsets_intersection.into_iter().collect();
                    hashsets_intersection.sort();
                    hi_intersection.sort();
                    assert_equal(hi_intersection, hashsets_intersection);
                }
            }

            // consume resumable intersection leftovers
            {
                let intersection = intersection_state.resume(hi_sets.iter());
                for (start_index, block) in intersection{
                    SimdOp::traverse_one_indices(block,
                        |index|{
                            let index = start_index + index;
                            initial_hashsets_intersection.remove(&index);
                            ControlFlow::Continue(())
                        }
                    );
                }
            }
            // assert that we consumed all initial intersection set.
            assert!(initial_hashsets_intersection.is_empty());
        }
    }

    #[test]
    fn empty_intersection_test(){
        let state = IntersectionBlocksState::default();
        let mut iter = state.resume(std::iter::empty());
        let next = iter.next();
        assert!(next.is_none());
    }

    #[test]
    fn one_intersection_test(){
        let mut hi_set = HiSparseBitset::default();
        hi_set.insert(0);
        hi_set.insert(12300);
        hi_set.insert(8760);
        hi_set.insert(521);

        let state = IntersectionBlocksState::default();
        let mut iter = state.resume([&hi_set].into_iter());

        let mut intersection = Vec::new();
        for (start_index, block) in iter{
            SimdOp::traverse_one_indices(block,
                |index|{
                    intersection.push(start_index+index);
                    ControlFlow::Continue(())
                }
            );
        }
        intersection.sort();
        assert_equal(intersection, [0, 521, 8760, 12300]);
    }

    #[test]
    fn regression_test1() {
        // worked only below 2^14=16384.
        // Probably because 128^2 = 16384.
        // Problem on switching level0 block.
        let mut sets_data = vec![
            vec![
                16384
            ],
            vec![
                16384
            ],
        ];

        let hash_sets: Vec<HashSet<usize>> =
            sets_data.clone().into_iter()
            .map(|data| data.into_iter().collect())
            .collect();
        let hi_sets: Vec<HiSparseBitset> =
            sets_data.clone().into_iter()
            .map(|data| data.into_iter().collect())
            .collect();

        let etalon_intersection = hash_sets[0].intersection(&hash_sets[1]);
        println!("etalon: {:?}", etalon_intersection);

        {

            let mut indices2 = Vec::new();
            let state = IntersectionBlocksState::default();
            let iter = state.resume(hi_sets.iter());
            for (start_index, block) in iter{
                SimdOp::traverse_one_indices(block,
                    |index|{
                        indices2.push(start_index+index);
                        ControlFlow::Continue(())
                    }
                );
            }
            println!("indices: {:?}", indices2);
            assert_equal(etalon_intersection, &indices2);
        }
    }

    #[test]
    fn remove_regression_test1() {
        let mut hi_set = HiSparseBitset::new();
        hi_set.insert(10000);
        hi_set.remove(10000);
        hi_set.insert(10000);

        let c= hi_set.contains(10000);
        assert!(c);
    }
}
*/