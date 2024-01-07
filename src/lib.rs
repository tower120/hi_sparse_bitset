#![cfg_attr(miri, feature(alloc_layout_extra) )]
//! Hierarchical sparse bitset. 
//! 
//! Memory consumption does not depends on max index inserted.
//! 
//! ![](https://github.com/tower120/hi_sparse_bitset/raw/main/doc/hisparsebitset-bg-white-50.png)
//! 
//! The very structure of [BitSet] acts as acceleration structure for
//! intersection operation. All operations are incredibly fast - see benchmarks.
//! (insert/contains in "traditional bitset" ballpark, intersection/union - orders of magnitude faster)
//! 
//! It is multi-level structure. Last level contains actual bit-data. Each previous level
//! have bitmask, where each bit corresponds to `!is_empty` of bitblock in next level. 
//! 
//! In addition to "non-empty-marker" bitmasks, there is pointers(indices) to non-empty blocks in next level.
//! In this way, only blocks with actual data allocated.
//! 
//! For inter-bitset operations, for example intersection:
//! * root level bitmasks AND-ed.
//! * resulting bitmask traversed for bits with 1.
//! * indexes of bits with 1, used for getting pointers to the next level for each bitset.
//! * repeat for next level until the data level, then for each next 1 bit in each level.
//! 
//! Bitmasks allow to cutoff empty tree/hierarchy branches early for intersection operation,
//! and traverse only actual data during iteration.
//!
//! In addition to this, during the inter-bitset operation, level1 blocks of
//! bitsets are cached for faster access. Empty blocks are skipped and not added
//! to the cache container, which algorithmically speeds up bitblock computations
//! at the data level.
//! This has observable effect in a merge operation between N non-intersecting
//! bitsets: without this optimization - the data level bitmask would be OR-ed N times;
//! with it - only once.
//! 
//! # Config
//! 
//! Max index [BitSet] can hold, depends on used bitblocks capacity.
//! The bigger the bitblocks - the higher [BitSet] index range.
//! The lower - the smaller memory footprint it has.
//! 
//! Max index for 64bit blocks = 262_144; for 256bit blocks = 16_777_216.
//! 
//! Use [BitSet] with predefined [config]:
//! ```
//! type BitSet = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_128bit>;
//! ```
//! 
//! # Inter-bitset operations
//! 
//! Inter-bitset operations can be applied between ANY [BitSetInterface].
//! Output of inter-bitset operations are lazy bitsets(which are [BitSetInterface]s too).
//! This means that you can combine different operations however you want
//! without ever materializing them into actual [BitSet].
//! 
//! Use [reduce()] to apply inter-bitset operation between elements of bitsets iterator.
//! 
//! Use [apply()]  to apply inter-bitset operation between two bitsets. Also [&], [|], [`^`], [-].
//! 
//! You can define your own inter-bitset operation, by implementing [BitSetOp].
//! 
//! [&]: std::ops::BitAnd
//! [|]: std::ops::BitOr
//! [`^`]: std::ops::BitXor
//! [-]: std::ops::Sub
//! 
//! # Cursor
//! 
//! [BitSetInterface] iterators can return [cursor()], pointing to current iterator position. 
//! You can use [Cursor] to move ANY [BitSetInterface] iterator to it's position with [move_to].
//! 
//! You can also build cursor from index.
//! 
//! [cursor()]: crate::iter::CachingIndexIter::cursor
//! [Cursor]: crate::iter::IndexCursor
//! [move_to]: crate::iter::CachingIndexIter::move_to
//! 
//! # Iterator::for_each
//! 
//! [BitSetInterface] iterators have [for_each] specialization and stable [try_for_each] version - [traverse].
//! For tight loops, traversing is observably faster then iterating.
//! 
//! [for_each]: std::iter::Iterator::for_each
//! [try_for_each]: std::iter::Iterator::try_for_each
//! [traverse]: crate::iter::CachingIndexIter::traverse
//! 
//! # TrustedHierarchy
//! 
//! TrustedHierarchy means that each raised bit in hierarchy bitblock
//! is guaranteed to correspond to non-empty block.
//! That may be not true for [difference] and [symmetric difference] operation result.
//! 
//! You can check if bitset has TrustedHierarchy with [BitSetBase::TRUSTED_HIERARCHY]. 
//! 
//! Bitsets with TrustedHierarchy are faster to compare with [Eq] and
//! have O(1) [is_empty()].
//!
//! [difference]: ops::Sub
//! [symmetric difference]: ops::Xor
//! [is_empty()]: BitSetInterface::is_empty
//! 
//! # DataBlocks
//! 
//! You can iterate [DataBlock]s instead of individual indices. DataBlocks can be moved, cloned
//! and iterated for indices.
//! 
//! # SIMD
//! 
//! 128 and 256 bit configurations use SIMD. Make sure you compile with simd support
//! enabled (`sse2` for _128bit, `avx` for _256bit) to achieve best performance.
//! _sse2 enabled by default in Rust for most desktop environments_ 
//! 
//! If you don't need "wide" configurations, you may disable default feature "simd".   

#[cfg(test)]
mod test;

mod primitive;
mod block;
mod level;
mod bit_block;
pub mod bit_queue;
mod bit_utils;
pub mod config;
pub mod ops;
mod reduce;
mod bitset_interface;
mod apply;
pub mod iter;
pub mod cache;

pub use primitive::Primitive;
pub use bitset_interface::{BitSetBase, BitSetInterface};
pub use apply::Apply;
pub use reduce::Reduce;

use std::ops::ControlFlow;
use std::mem::{ManuallyDrop, MaybeUninit};
use config::Config;
use block::Block;
use level::Level;
use ops::BitSetOp;
pub use bit_block::BitBlock;
use bit_queue::BitQueue;
use cache::ReduceCache;
use bitset_interface::{LevelMasks, LevelMasksExt};

/// Use any other operation then intersection(and) require
/// to either do checks on block access (in LevelMasks), or
/// have one empty block at each level as default, and default indices pointing at it.
/// Second variant is in use now.
const INTERSECTION_ONLY: bool = false;

macro_rules! assume {
    ($e: expr) => {
        if !($e){
            std::hint::unreachable_unchecked();
        }
    };
}
pub(crate) use assume;

#[inline]
fn level_indices<Conf: Config>(index: usize) -> (usize/*level0*/, usize/*level1*/, usize/*data*/){
    // this should be const and act as const.
    /*const*/ let data_block_capacity_pot_exp  : usize = Conf::DataBitBlock::SIZE_POT_EXPONENT;
    /*const*/ let data_block_capacity          : usize = 1 << data_block_capacity_pot_exp;

    /*const*/ let level1_block_capacity_pot_exp: usize = Conf::Level1BitBlock::SIZE_POT_EXPONENT
                                                       + Conf::DataBitBlock::SIZE_POT_EXPONENT;
    /*const*/ let level1_block_capacity        : usize = 1 << level1_block_capacity_pot_exp;

    // index / LEVEL1_BLOCK_CAP
    let level0 = index >> level1_block_capacity_pot_exp;
    // index % LEVEL1_BLOCK_CAP
    let level0_remainder = index & (level1_block_capacity - 1);

    // level0_remainder / DATA_BLOCK_CAP
    let level1 = level0_remainder >> data_block_capacity_pot_exp;

    // level0_remainder % DATA_BLOCK_CAP = index % LEVEL1_BLOCK_CAP % DATA_BLOCK_CAP
    let level1_remainder = index & (
        (level1_block_capacity-1) & (data_block_capacity-1)
    );

    let data = level1_remainder;

    (level0, level1, data)
}

type Level1Block<Conf> = Block<
    <Conf as Config>::Level1BitBlock,
    <Conf as Config>::DataBlockIndex,
    <Conf as Config>::Level1BlockIndices
>;
type LevelDataBlock<Conf> = Block<
    <Conf as Config>::DataBitBlock, usize, [usize;0]
>;

type Level1<Conf> = Level<
    <Conf as Config>::Level1BitBlock,
    <Conf as Config>::DataBlockIndex,
    <Conf as Config>::Level1BlockIndices
>;
type LevelData<Conf> = Level<
    <Conf as Config>::DataBitBlock, usize, [usize;0]
>;

/// Hierarchical sparse bitset.
///
/// Tri-level hierarchy. Highest uint it can hold
/// is Level0Mask::BITS * Level1Mask::BITS * DenseBlock::BITS.
///
/// Only last level contains blocks of actual data. Empty(skipped) data blocks
/// are not allocated.
///
/// Structure optimized for intersection speed. 
/// _(Other inter-bitset operations are in fact fast too - but intersection has lowest algorithmic complexity.)_
/// Insert/remove/contains is fast O(1) too.
pub struct BitSet<Conf: Config>{
    level0: Block<Conf::Level0BitBlock, Conf::Level1BlockIndex, Conf::Level0BlockIndices>,
    level1: Level1<Conf>,
    data  : LevelData<Conf>,
}

impl<Conf: Config> Default for BitSet<Conf> {
    #[inline]
    fn default() -> Self{
        Self{
            level0: Default::default(),
            level1: Default::default(),
            data: Default::default(),
        }
    }
}

impl<Conf: Config> Clone for BitSet<Conf> {
    fn clone(&self) -> Self {
        Self{
            level0: self.level0.clone(),
            level1: self.level1.clone(),
            data: self.data.clone(),
        }
    }
}

impl<Conf> BitSet<Conf> 
where
    Conf: Config
{
    #[inline]
    pub fn new() -> Self{
        Self::default()
    }

    #[inline]
    fn is_in_range(index: usize) -> bool{
        index < Conf::max_value()
    }

    #[inline]
    fn get_block_indices(&self, level0_index: usize, level1_index: usize)
        -> Option<(usize, usize)>
    {
        // 1. Level0
        let level1_block_index = unsafe{
            self.level0.get(level0_index)?
        }.as_usize();

        // 2. Level1
        let data_block_index = unsafe{
            let level1_block = self.level1.blocks().get_unchecked(level1_block_index);
            level1_block.get(level1_index)?
        }.as_usize();

        Some((level1_block_index, data_block_index))
    }

    /// # Safety
    ///
    /// Will panic, if `index` is out of range.
    pub fn insert(&mut self, index: usize){
        assert!(Self::is_in_range(index), "index out of range!");

        // That's indices to next level
        let (level0_index, level1_index, data_index) = level_indices::<Conf>(index);

        // 1. Level0
        let level1_block_index = unsafe{
            self.level0.get_or_insert(level0_index, ||{
                let block_index = self.level1.insert_block();
                Conf::Level1BlockIndex::from_usize(block_index)
            })
        }.as_usize();

        // 2. Level1
        let data_block_index = unsafe{
            let level1_block = self.level1.blocks_mut().get_unchecked_mut(level1_block_index);
            level1_block.get_or_insert(level1_index, ||{
                let block_index = self.data.insert_block();
                Conf::DataBlockIndex::from_usize(block_index)
            })
        }.as_usize();

        // 3. Data level
        unsafe{
            let data_block = self.data.blocks_mut().get_unchecked_mut(data_block_index);
            data_block.insert_mask_unchecked(data_index);
        }
    }

    /// Returns false if index is invalid/not in bitset.
    pub fn remove(&mut self, index: usize) -> bool {
        if !Self::is_in_range(index){
            return false;
        }

        // 1. Resolve indices
        let (level0_index, level1_index, data_index) = level_indices::<Conf>(index);
        let (level1_block_index, data_block_index) = match self.get_block_indices(level0_index, level1_index){
            None => return false,
            Some(value) => value,
        };

        unsafe{
            // 2. Get Data block and set bit
            let data_block = self.data.blocks_mut().get_unchecked_mut(data_block_index);
            let existed = data_block.remove(data_index);
            
            // TODO: fast check of mutated data_block's primitive == 0?  

            //if existed{
                // 3. Remove free blocks
                if data_block.is_empty(){
                    // remove data block
                    self.data.remove_empty_block_unchecked(data_block_index);

                    // remove pointer from level1
                    let level1_block = self.level1.blocks_mut().get_unchecked_mut(level1_block_index);
                    level1_block.remove(level1_index);

                    if level1_block.is_empty(){
                        // remove level1 block
                        self.level1.remove_empty_block_unchecked(level1_block_index);

                        // remove pointer from level0
                        self.level0.remove(level0_index);
                    }
                }
            //}
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
        unsafe{ assume!(ok); }
    }
}

impl<Conf: Config> FromIterator<usize> for BitSet<Conf> {
    fn from_iter<T: IntoIterator<Item=usize>>(iter: T) -> Self {
        let mut this = Self::default();
        for i in iter{
            this.insert(i);
        }
        this
    }
}

impl<Conf: Config, const N: usize> From<[usize; N]> for BitSet<Conf> {
    fn from(value: [usize; N]) -> Self {
        Self::from_iter(value.into_iter())
    }
}

impl<Conf: Config> BitSetBase for BitSet<Conf>{
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = true;
}

impl<Conf: Config> LevelMasks for BitSet<Conf>{
    #[inline]
    fn level0_mask(&self) -> Conf::Level0BitBlock {
        *self.level0.mask()
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize) -> Conf::Level1BitBlock {
        let level1_block_index = self.level0.get_unchecked(level0_index).as_usize();
        let level1_block = self.level1.blocks().get_unchecked(level1_block_index);
        *level1_block.mask()
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize) -> Conf::DataBitBlock {
        let level1_block_index = self.level0.get_unchecked(level0_index).as_usize();
        let level1_block = self.level1.blocks().get_unchecked(level1_block_index);

        let data_block_index = level1_block.get_unchecked(level1_index).as_usize();
        let data_block = self.data.blocks().get_unchecked(data_block_index);
        *data_block.mask()
    }
}

impl<Conf: Config> LevelMasksExt for BitSet<Conf>{
    /// Points to elements in heap.
    type Level1Blocks = (*const LevelDataBlock<Conf> /* array pointer */, *const Level1Block<Conf>);

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
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool){
        let level1_block_index = self.level0.get_unchecked(level0_index);

        // TODO: This can point to static empty block, if level1_block_index invalid.

        let level1_block = self.level1.blocks().get_unchecked(level1_block_index.as_usize());
        level1_blocks.write((self.data.blocks().as_ptr(), level1_block));
        (*level1_block.mask(), !level1_block_index.is_zero())
    }

    #[inline]
    unsafe fn data_mask_from_blocks(
        /*&self,*/ level1_blocks: &Self::Level1Blocks, level1_index: usize
    ) -> Conf::DataBitBlock {
        let array_ptr = level1_blocks.0;
        let level1_block = &*level1_blocks.1;

        let data_block_index = level1_block.get_unchecked(level1_index);
        let data_block = &*array_ptr.add(data_block_index.as_usize());
        *data_block.mask()
    }
}

#[inline]
fn data_block_start_index<Conf: Config>(level0_index: usize, level1_index: usize) -> usize{
    let level0_offset = level0_index << (Conf::DataBitBlock::SIZE_POT_EXPONENT + Conf::Level1BitBlock::SIZE_POT_EXPONENT);
    let level1_offset = level1_index << (Conf::DataBitBlock::SIZE_POT_EXPONENT);
    level0_offset + level1_offset
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataBlock<Block>{
    pub start_index: usize,
    pub bit_block: Block
}
impl<Block: BitBlock> DataBlock<Block>{
    // TODO: remove
    /// traverse approx. 15% faster then iterator
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
    
    /// Calculate elements count in DataBlock.
    /// 
    /// On most platforms, this should be faster then manually traversing DataBlock
    /// and counting elements. It use hardware supported popcnt operations,
    /// whenever possible. 
    #[inline]
    pub fn len(&self) -> usize {
        self.bit_block.count_ones()
    }
    
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bit_block.is_zero()
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

#[derive(Clone)]
pub struct DataBlockIter<Block: BitBlock>{
    start_index: usize,
    bit_block_iter: Block::BitsIter
}
impl<Block: BitBlock> DataBlockIter<Block>{
    /// Stable version of [try_for_each].
    /// 
    /// traverse approx. 15% faster then iterator
    /// 
    /// [try_for_each]: std::iter::Iterator::try_for_each
    #[inline]
    pub fn traverse<F>(self, mut f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>
    {
        self.bit_block_iter.traverse(|index| f(self.start_index + index))
    }    
}
impl<Block: BitBlock> Iterator for DataBlockIter<Block>{
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.bit_block_iter.next().map(|index|self.start_index + index)
    }

    #[inline]
    fn for_each<F>(self, mut f: F)
    where
        F: FnMut(Self::Item)
    {
        self.traverse(|index| {
            f(index);
            ControlFlow::Continue(())
        });
    }
}

/// Creates a lazy bitset, as [BitSetOp] application between two bitsets.
#[inline]
pub fn apply<Op, S1, S2>(op: Op, s1: S1, s2: S2) -> Apply<Op, S1, S2>
where
    Op: BitSetOp,
    S1: BitSetInterface,
    S2: BitSetInterface<Conf = <S1 as BitSetBase>::Conf>,
{
    Apply::new(op, s1, s2)
}

/// Creates a lazy bitset, as bitsets iterator reduction.
///
/// "Reduce" term used in Rust's [Iterator::reduce] sense.
///
/// If the `bitsets` is empty - returns `None`; otherwise - returns the resulting
/// lazy bitset.
///
/// `bitsets` iterator must be cheap to clone (slice iterator is a good example).
/// It will be cloned AT LEAST once for each returned [DataBlock] during iteration.
///
/// # Safety
///
/// Panics, if [Config::DefaultCache] capacity is smaller then sets len.
#[inline]
pub fn reduce<Conf, Op, I>(op: Op, bitsets: I)
   -> Option<reduce::Reduce<Op, I, Conf::DefaultCache>>
where
    Conf: Config,
    Op: BitSetOp,
    I: Iterator + Clone,
    I::Item: BitSetInterface<Conf = Conf>,
{
    reduce_w_cache(op, bitsets, Default::default())
}

/// [reduce], using specific [cache] for iteration.
///
/// Cache applied to current operation only, so you can combine different cache
/// types. 
/// 
/// N.B. Alternatively, you can change [Config::DefaultCache] and use [reduce].
///
/// # Safety
///
/// Panics, if `Cache` capacity is smaller then sets len.
/// 
/// [reduce]: reduce()
#[inline]
pub fn reduce_w_cache<Op, I, Cache>(_: Op, bitsets: I, _: Cache)
    -> Option<reduce::Reduce<Op, I, Cache>>
where
    Op: BitSetOp,
    I: Iterator + Clone,
    I::Item: BitSetInterface,
    Cache: ReduceCache
{
    // Compile-time if
    if Cache::MAX_LEN != usize::MAX{
        let len = bitsets.clone().count();
        assert!(len<=Cache::MAX_LEN, "Cache is too small for this iterator.");
        if len == 0{
            return None;
        }
    } else {
        if bitsets.clone().next().is_none(){
            return None;
        }
    }

    Some(reduce::Reduce{ sets: bitsets, phantom: Default::default() })
}

// TODO: Do we need fold as well?