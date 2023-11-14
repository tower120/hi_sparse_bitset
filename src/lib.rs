#![cfg_attr(miri, feature(alloc_layout_extra) )]

mod block;
mod level;
mod bit_block;
mod bit_queue;
mod bit_op;
pub mod configs;
pub mod binary_op;
mod reduce;
mod bitset_interface;
mod op;
pub mod iter;
pub mod cache;

#[cfg(test)]
mod test;

use std::{ops::ControlFlow};
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::{BitAndAssign, BitXorAssign};
use num_traits::{AsPrimitive, PrimInt, WrappingNeg, Zero};

use block::Block;
use level::Level;
use crate::binary_op::BinaryOp;
use crate::bit_block::BitBlock;
use crate::bit_queue::BitQueue;
use crate::iter::BlockIterator;
use crate::reduce::ReduceCacheImplBuilder;
use crate::bitset_interface::{LevelMasks, LevelMasksExt};

pub use bitset_interface::BitSetInterface;
pub use op::BitSetOp;
pub use reduce::Reduce;


/// Use any other operation then intersection(and) require
/// to either do checks on block access (in LevelMasks), or
/// have one empty block at each level as default, and default indices pointing at it.
/// Second variant in use now.
const INTERSECTION_ONLY: bool = false;

pub trait Primitive: PrimInt + AsPrimitive<usize> + BitAndAssign + BitXorAssign + WrappingNeg + Default + 'static {}
impl<T: PrimInt + AsPrimitive<usize> + BitAndAssign + BitXorAssign + WrappingNeg + Default + 'static> Primitive for T{}

pub trait IConfig: 'static {
    type Level0BitBlock: BitBlock + Default;
    /// Must be big enough to accommodate at least Level0BitBlock::SIZE
    /// Must be [Self::Level1BlockIndex; 1 << Level0BitBlock::SIZE_POT_EXPONENT]
    type Level0BlockIndices: AsRef<[Self::Level1BlockIndex]> + AsMut<[Self::Level1BlockIndex]> + Clone;

    type Level1BitBlock: BitBlock + Default;
    type Level1BlockIndex: Primitive;
    /// Must be big enough to accommodate at least Level1BitBlock::SIZE.
    /// Must be [Self::DataBlockIndex; 1 << Level1BitBlock::SIZE_POT_EXPONENT]
    type Level1BlockIndices: AsRef<[Self::DataBlockIndex]> + AsMut<[Self::DataBlockIndex]> + Clone;

    type DataBitBlock: BitBlock + Default;
    /// Should be big enough to accommodate at least `max_range<Config>() / DataBitBlock::SIZE`
    type DataBlockIndex: Primitive;

    // TODO: remove this?
    // There can be BlockIteratorBuilder as well, but parameterized
    // Iter works too for now.
    type DefaultBlockIterator<T: LevelMasksExt>: BlockIterator<BitSet = T>;
    type DefaultCache: ReduceCacheImplBuilder;
}

// TODO: move somewhere more appropriate
#[inline]
fn data_block_start_index<Config: IConfig>(level0_index: usize, level1_index: usize) -> usize{
    let level0_offset = level0_index << (Config::DataBitBlock::SIZE_POT_EXPONENT + Config::Level1BitBlock::SIZE_POT_EXPONENT);
    let level1_offset = level1_index << (Config::DataBitBlock::SIZE_POT_EXPONENT);
    level0_offset + level1_offset
}

#[inline]
fn level_indices<Config: IConfig>(index: usize) -> (usize/*level0*/, usize/*level1*/, usize/*data*/){
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

/// Max usize, [BitSet] with `Config` can hold.
pub const fn max_range<Config: IConfig>() -> usize {
    let mut max_range = (1 << Config::Level0BitBlock::SIZE_POT_EXPONENT)
        * (1 << Config::Level1BitBlock::SIZE_POT_EXPONENT)
        * (1 << Config::DataBitBlock::SIZE_POT_EXPONENT);

    if !INTERSECTION_ONLY{
        // We occupy one block for "empty" at each level, except root.
        max_range
            - (1 << Config::Level1BitBlock::SIZE_POT_EXPONENT)
            - (1 << Config::DataBitBlock::SIZE_POT_EXPONENT);
    }

    max_range
}

type Level1Block<Config> = Block<
    <Config as IConfig>::Level1BitBlock,
    <Config as IConfig>::DataBlockIndex,
    <Config as IConfig>::Level1BlockIndices
>;

type LevelDataBlock<Config> = Block<
    <Config as IConfig>::DataBitBlock, usize, [usize;0]
>;

/// Hierarchical sparse bitset.
///
/// Tri-level hierarchy. Highest uint it can hold
/// is Level0Mask * Level1Mask * DenseBlock.
///
/// Only last level contains blocks of actual data. Empty(skipped) data blocks
/// are not allocated.
///
/// Structure optimized for intersection speed. Insert/remove/contains is fast O(1) too.
pub struct BitSet<Config: IConfig>{
    level0: Block<Config::Level0BitBlock, Config::Level1BlockIndex, Config::Level0BlockIndices>,
    level1: Level<Level1Block<Config>,    Config::Level1BlockIndex>,
    data  : Level<LevelDataBlock<Config>, Config::DataBlockIndex>,
}

impl<Config: IConfig> Default for BitSet<Config> {
    #[inline]
    fn default() -> Self{
        Self{
            level0: Default::default(),
            level1: Default::default(),
            data: Default::default(),
        }
    }
}

impl<Config: IConfig> Clone for BitSet<Config> {
    fn clone(&self) -> Self {
        Self{
            level0: self.level0.clone(),
            level1: self.level1.clone(),
            data: self.data.clone(),
        }
    }
}

impl<Config: IConfig> BitSet<Config> {
    #[inline]
    pub fn new() -> Self{
        Self::default()
    }

    #[inline]
    fn is_in_range(index: usize) -> bool{
        index < max_range::<Config>()
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
        let (level0_index, level1_index, data_index) = level_indices::<Config>(index);

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
        let (level0_index, level1_index, data_index) = level_indices::<Config>(index);
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
}

impl<Config: IConfig> FromIterator<usize> for BitSet<Config> {
    fn from_iter<T: IntoIterator<Item=usize>>(iter: T) -> Self {
        let mut this = Self::default();
        for i in iter{
            this.insert(i);
        }
        this
    }
}

impl<Config: IConfig, const N: usize> From<[usize; N]> for BitSet<Config> {
    fn from(value: [usize; N]) -> Self {
        Self::from_iter(value.into_iter())
    }
}

impl<Config: IConfig> LevelMasks for BitSet<Config>{
    type Config = Config;

    #[inline]
    fn level0_mask(&self) -> Config::Level0BitBlock {
        *self.level0.mask()
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize) -> Config::Level1BitBlock {
        let level1_block_index = self.level0.get_unchecked(level0_index);
        let level1_block = self.level1.blocks().get_unchecked(level1_block_index.as_());
        *level1_block.mask()
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize) -> Config::DataBitBlock {
        let level1_block_index = self.level0.get_unchecked(level0_index);
        let level1_block = self.level1.blocks().get_unchecked(level1_block_index.as_());

        let data_block_index = level1_block.get_unchecked(level1_index);
        let data_block = self.data.blocks().get_unchecked(data_block_index.as_());
        *data_block.mask()
    }
}

impl<Config: IConfig> LevelMasksExt for BitSet<Config>{
    /// Points to elements in heap.
    type Level1Blocks = (*const LevelDataBlock<Config> /* array pointer */, *const Level1Block<Config>);

    const EMPTY_LVL1_TOLERANCE: bool = true;

    type CacheData = ();
    fn make_cache(&self) -> Self::CacheData { () }
    fn drop_cache(&self, _: &mut ManuallyDrop<Self::CacheData>) {}

    #[inline]
    unsafe fn update_level1_blocks(
        &self,
        _: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks>,
        level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool){
        let level1_block_index = self.level0.get_unchecked(level0_index);

        // TODO: This can point to static empty block, if level1_block_index invalid.

        let level1_block = self.level1.blocks().get_unchecked(level1_block_index.as_());
        level1_blocks.write((self.data.blocks().as_ptr(), level1_block));
        (*level1_block.mask(), !level1_block_index.is_zero())
    }

    #[inline]
    unsafe fn data_mask_from_blocks(
        /*&self,*/ level1_blocks: &Self::Level1Blocks, level1_index: usize
    ) -> Config::DataBitBlock {
        let array_ptr = level1_blocks.0;
        let level1_block = &*level1_blocks.1;

        let data_block_index = level1_block.get_unchecked(level1_index);
        let data_block = &*array_ptr.add(data_block_index.as_());
        *data_block.mask()
    }
}

#[derive(Clone, Debug)]
pub struct DataBlock<Block>{
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

    #[inline]
    pub fn iter(&self) -> DataBlockIter<Block>{
        DataBlockIter{
            start_index: self.start_index,
            bit_block_iter: self.bit_block.clone().bits_iter()
        }
    }
}
impl<Block: BitBlock> IntoIterator for DataBlock<Block>{
    type Item = usize;
    type IntoIter = DataBlockIter<Block>;

    /// This is actually no-op fast.
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        DataBlockIter{
            start_index: self.start_index,
            bit_block_iter: self.bit_block.bits_iter()
        }
    }
}
pub struct DataBlockIter<Block: BitBlock>{
    start_index: usize,
    bit_block_iter: Block::BitsIter
}
impl<Block: BitBlock> DataBlockIter<Block>{
    #[inline]
    pub(crate) fn empty() -> Self{
        Self{ start_index: 0, bit_block_iter: BitQueue::empty() }
    }
}
impl<Block: BitBlock> Iterator for DataBlockIter<Block>{
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.bit_block_iter.next().map(|index|self.start_index + index)
    }
}

/// Creates a virtual bitset, as [BinaryOp] application between two sets.
#[inline]
pub fn apply<Op, S1, S2>(op: Op, s1: S1, s2: S2) -> BitSetOp<Op, S1, S2>
where
    Op: BinaryOp,
    S1: BitSetInterface,
    S2: BitSetInterface<Config = <S1 as BitSetInterface>::Config>,
{
    BitSetOp::new(op, s1, s2)
}

/// Creates a virtual bitset, as sets iterator reduction.
///
/// If the `sets` is empty - returns `None`; otherwise - returns the resulting
/// virtual bitset.
///
/// `sets` iterator must be cheap to clone (slice iterator is good example).
/// It will be cloned AT LEAST once for each returned [DataBlock] during iteration.
///
/// # Safety
///
/// Panics during iteration, if [Config::DefaultCache] is smaller then sets len.
#[inline]
pub fn reduce<Config, Op, S>(op: Op, sets: S)
    -> Option<reduce::Reduce<Op, S, Config::DefaultCache>>
where
    Config:IConfig,
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: BitSetInterface<Config = Config>,
{
    reduce_w_cache(op, sets, Default::default())
}

/// [reduce], using specific `Cache` for iteration.
///
/// Cache applied to current operation only, so you can combine different cache
/// types. Alternatively, you can change [Config::DefaultCache] and use [reduce()].
///
/// See [mod@cache].
///
/// # Safety
///
/// Panics during iteration, if Cache is smaller then sets len.
///
/// [reduce]: reduce()
#[inline]
pub fn reduce_w_cache<Op, S, Cache>(_: Op, sets: S, _: Cache)
    -> Option<reduce::Reduce<Op, S, Cache>>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: BitSetInterface,
{
    if sets.clone().next().is_none(){
        return None;
    }
    Some(reduce::Reduce{ sets, phantom: Default::default() })
}

// TODO: Do we need fold as well?