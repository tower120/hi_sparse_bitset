pub mod intersection_blocks_resumable;
mod block;
mod level;
mod bit_block;
mod bit_queue;
mod bit_op;
pub mod configs;

#[cfg(test)]
mod test;

use std::{ops, ops::ControlFlow};
use std::ops::{BitAndAssign, BitXorAssign};
use num_traits::{AsPrimitive, PrimInt, WrappingNeg};

use block::Block;
use level::Level;
use crate::bit_block::BitBlock;

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

pub trait MyPrimitive: PrimInt + AsPrimitive<usize> + BitAndAssign + BitXorAssign + WrappingNeg + Default + 'static {}
impl<T: PrimInt + AsPrimitive<usize> + BitAndAssign + BitXorAssign + WrappingNeg + Default + 'static> MyPrimitive for T{}

pub trait IConfig: 'static {
    type Level0BitBlock: BitBlock + Default;
    /// Must be big enough to accommodate at least Level0BitBlock::SIZE
    /// Must be [Self::Level1BlockIndex; 1 << Level0BitBlock::SIZE_POT_EXPONENT]
    type Level0BlockIndices: AsRef<[Self::Level1BlockIndex]> + AsMut<[Self::Level1BlockIndex]> + Clone;

    type Level1BitBlock: BitBlock + Default;
    type Level1BlockIndex: MyPrimitive;
    /// Must be big enough to accommodate at least Level1BitBlock::SIZE.
    /// Must be [Self::DataBlockIndex; 1 << Level1BitBlock::SIZE_POT_EXPONENT]
    type Level1BlockIndices: AsRef<[Self::DataBlockIndex]> + AsMut<[Self::DataBlockIndex]> + Clone;

    type DataBitBlock: BitBlock + Default;
    /// Should be big enough to accommodate at least `max_range<Config>() / DataBitBlock::SIZE`
    type DataBlockIndex: MyPrimitive;
}

pub const fn max_range<Config: IConfig>() -> usize {
    (1 << Config::Level0BitBlock::SIZE_POT_EXPONENT)
    * (1 << Config::Level1BitBlock::SIZE_POT_EXPONENT)
    * (1 << Config::DataBitBlock::SIZE_POT_EXPONENT)
}

/// Hierarchical sparse bitset. Tri-level hierarchy. Highest uint it can hold is Level0Mask * Level1Mask * DenseBlock.
/// 
/// Only last level contains blocks of actual data. Empty(skipped) data blocks are not allocated.
/// 
/// Structure optimized for intersection speed. Insert/remove/contains is fast O(1) too.
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
}

impl<Config: IConfig> Default for HiSparseBitset<Config>
// TODO: consider changing this somehow
where
    usize: AsPrimitive<Config::Level1BlockIndex>,
    usize: AsPrimitive<Config::DataBlockIndex>,
{
    #[inline]
    fn default() -> Self{
        Self{
            level0: Default::default(),
            level1: Default::default(),
            data: Default::default(),
        }
    }
}

impl<Config: IConfig> Clone for HiSparseBitset<Config>
// TODO: consider changing this somehow
    where
        usize: AsPrimitive<Config::Level1BlockIndex>,
        usize: AsPrimitive<Config::DataBlockIndex>,
{
    fn clone(&self) -> Self {
        Self{
            level0: self.level0.clone(),
            level1: self.level1.clone(),
            data: self.data.clone(),
        }
    }
}

impl<Config: IConfig> HiSparseBitset<Config>
// TODO: consider changing this somehow
where
    usize: AsPrimitive<Config::Level1BlockIndex>,
    usize: AsPrimitive<Config::DataBlockIndex>,
{
    #[inline]
    pub fn new() -> Self{
        Self::default()
    }

    #[inline]
    fn is_in_range(index: usize) -> bool{
        index < max_range::<Config>()
    }

    #[inline]
    fn level_indices(index: usize) -> (usize/*level0*/, usize/*level1*/, usize/*data*/){
        // this should be const and act as const.
        // const DATA_BLOCK_SIZE:  usize = 1 << DenseBlock::SIZE_POT_EXPONENT;
        let DATA_BLOCK_CAPACITY_POT_EXP:  usize = Config::DataBitBlock::SIZE_POT_EXPONENT;
        // const LEVEL1_BLOCK_SIZE: usize = (1 << Level1Mask::SIZE_POT_EXPONENT) * DATA_BLOCK_SIZE;
        let LEVEL1_BLOCK_CAPACITY_POT_EXP: usize = Config::Level1BitBlock::SIZE_POT_EXPONENT
                                                 + Config::DataBitBlock::SIZE_POT_EXPONENT;

        // index / LEVEL1_BLOCK_SIZE
        let level0 = index >> LEVEL1_BLOCK_CAPACITY_POT_EXP;
        // TODO: use remainder % trick here
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

    /// # Safety
    ///
    /// Will panic, if `index` is out of range.
    pub fn insert(&mut self, index: usize){
        assert!(Self::is_in_range(index), "index out of range!");

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
        if !Self::is_in_range(index){
            return false;
        }

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

impl<Config: IConfig> FromIterator<usize> for HiSparseBitset<Config>
where
    usize: AsPrimitive<Config::Level1BlockIndex>,
    usize: AsPrimitive<Config::DataBlockIndex>,
{
    fn from_iter<T: IntoIterator<Item=usize>>(iter: T) -> Self {
        let mut this = Self::default();
        for i in iter{
            this.insert(i);
        }
        this
    }
}

// TODO: use it
pub struct DataBlock<Block: BitBlock>{
    pub start_index: usize,
    pub bit_block: Block
}
impl<Block: BitBlock> DataBlock<Block>{
    #[inline]
    pub fn traverse<F>(&self, mut f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>
    {
        self.bit_block.traverse_bits(|index| f(self.start_index + index))
    }
}

// TODO: Consider using &IntoIterator instead of cloning iterator?
// See doc/HiSparseBitset.png for illustration.
//
// On each level We first calculate intersection mask between all sets, 
// then depth traverse only intersected elements/indices/blocks.
/// `sets` iterator will be cloned multiple times.
pub fn intersection_blocks_traverse<'a, S, F, Config: IConfig + 'a>(sets: S, mut foreach_block: F)
where
    S: IntoIterator<Item = &'a HiSparseBitset<Config>>,
    S::IntoIter: Clone,
    F: FnMut(usize/*block_start_index*/, Config::DataBitBlock),

    usize: AsPrimitive<Config::Level1BlockIndex>,
    usize: AsPrimitive<Config::DataBlockIndex>,
{
    use ControlFlow::*;
    let sets = sets.into_iter();

    // Level0
    let level0_intersection = 
        sets.clone()
        .map(|set| *set.level0.mask())
        .reduce(ops::BitAnd::bitand);

    let level0_intersection = match level0_intersection{
        Some(intersection) => intersection,
        None => return,
    };
    if level0_intersection.is_zero(){
        return;
    }

    level0_intersection.traverse_bits(
        |level0_index| level1_intersection_traverse(sets.clone(), level0_index, &mut foreach_block)
    );

    // Level1
    #[inline]
    fn level1_intersection_traverse<'a, Config: IConfig + 'a>(
        sets: impl Iterator<Item = &'a HiSparseBitset<Config>> + Clone,
        level0_index: usize, 
        foreach_block: &mut impl FnMut(usize/*block_start_index*/, Config::DataBitBlock)
    ) -> ControlFlow<()>
    where
        usize: AsPrimitive<Config::Level1BlockIndex>,
        usize: AsPrimitive<Config::DataBlockIndex>,
    {
        let level1_intersection = unsafe{
            sets.clone()
            .map(|set| {
                let level1_block_index = set.level0.get_unchecked(level0_index);
                let level1_block = set.level1.blocks().get_unchecked(level1_block_index.as_());
                *level1_block.mask()
            })
            .reduce(ops::BitAnd::bitand)
            .unwrap_unchecked()
        };

        level1_intersection.traverse_bits(
            |level1_index| data_intersection_traverse(sets.clone(), level0_index, level1_index, foreach_block)
        );

        Continue(())
    }

    // Data
    #[inline]
    fn data_intersection_traverse<'a, Config: IConfig + 'a>(
        sets: impl Iterator<Item = &'a HiSparseBitset<Config>>,
        level0_index: usize, 
        level1_index: usize,
        foreach_block: &mut impl FnMut(usize/*block_start_index*/, Config::DataBitBlock)
    ) -> ControlFlow<()>
    where
        usize: AsPrimitive<Config::Level1BlockIndex>,
        usize: AsPrimitive<Config::DataBlockIndex>,
    {
        let data_intersection = unsafe{
            sets
            .map(|set| {
                // We could collect level1_block_index/&level1_block during level1 walk,
                // but benchmarks showed that does not have measurable performance benefits.

                let level1_block_index = set.level0.get_unchecked(level0_index);
                let level1_block       = set.level1.blocks().get_unchecked(level1_block_index.as_());

                let data_block_index   = level1_block.get_unchecked(level1_index);
                *set.data.blocks().get_unchecked(data_block_index.as_()).mask()
            })
            .reduce(ops::BitAnd::bitand)
            .unwrap_unchecked()
        };

        let block_start_index = (level0_index << (Config::DataBitBlock::SIZE_POT_EXPONENT + Config::Level1BitBlock::SIZE_POT_EXPONENT))
                              + (level1_index << (Config::DataBitBlock::SIZE_POT_EXPONENT));

        (foreach_block)(block_start_index, data_intersection);

        Continue(())
    }
}

/// Same as [intersection_blocks_traverse], but iterator, and a tiny bit slower.
/// 
/// `sets` iterator will be cloned and iterated multiple times.
#[inline]
pub fn intersection_blocks<'a, Config, S>(sets: S)
    -> intersection_blocks_resumable::IntersectionBlocks<'a, Config, S::IntoIter>
where
    Config: IConfig,
    S: IntoIterator<Item = &'a HiSparseBitset<Config>>,
    S::IntoIter: Clone,

    usize: AsPrimitive<Config::Level1BlockIndex>,
    usize: AsPrimitive<Config::DataBlockIndex>,
{
    intersection_blocks_resumable::IntersectionBlocks::new(sets.into_iter())
}

/// For Debug purposes.
pub fn collect_intersection<Config: IConfig>(sets: &[HiSparseBitset<Config>]) -> Vec<usize>
where
    usize: AsPrimitive<Config::Level1BlockIndex>,
    usize: AsPrimitive<Config::DataBlockIndex>,
{
    use ControlFlow::*;
    let mut indices = Vec::new();
    intersection_blocks_traverse(sets, 
        |start_index, block|{
            block.traverse_bits(
                |index|{
                    indices.push(start_index+index);
                    Continue(())
                }
            );
        }
    );
    indices
}
