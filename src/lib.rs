#![cfg_attr(miri, feature(alloc_layout_extra) )]
#![cfg_attr(docsrs, feature(doc_cfg))]
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
//! In addition to this, during the [reduce] operation, level1 blocks of
//! bitsets are cached for faster access. Empty blocks are skipped and not added
//! to the cache container, which algorithmically speeds up bitblock computations
//! at the data level.
//! This has an observable effect on a merge operation between N non-intersecting
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
//! [BitSetInterface] iterators can return [cursor()], pointing to the current iterator position. 
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
//! # Custom bitsets
//! 
//! You can make your own bitsets - like 
//! generative sets (empty, full), specially packed sets (range-fill), 
//! adapters, etc. See [internals] module. You need `impl` feature for that.
//! 
//! # SIMD
//! 
//! 128 and 256 bit configurations use SIMD powered by [wide]. Make sure you compile with simd support
//! enabled (`sse2` for _128bit, `avx` for _256bit) to achieve best performance.
//! _sse2 enabled by default in Rust for most desktop environments_ 
//!
//! If you want to use other SIMD types/registers - see [internals] module.
//! If you don't need "wide" configurations, you may disable default feature `simd`.
//!
//! [wide]: https://crates.io/crates/wide

#[cfg(test)]
mod test;

mod primitive;
mod block;
mod level;
mod bit_block;
mod bit_queue;
mod bit_utils;
pub mod config;
pub mod ops;
mod reduce;
mod bitset_interface;
mod apply;
pub mod iter;
pub mod cache;
pub mod internals;
mod bitset_ranges;

pub use bitset_interface::{BitSetBase, BitSetInterface};
pub use bitset_ranges::BitSetRanges;
pub use apply::Apply;
pub use reduce::Reduce;
pub use bit_block::BitBlock;

use std::ops::ControlFlow;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ptr::NonNull;
use primitive::Primitive;
use config::max_addressable_index;
use config::Config;
use block::Block;
use level::Level;
use ops::BitSetOp;
use bit_queue::BitQueue;
use cache::ReduceCache;
use bitset_interface::{LevelMasks, LevelMasksIterExt};

macro_rules! assume {
    ($e: expr) => {
        if !($e){
            std::hint::unreachable_unchecked();
        }
    };
}
pub(crate) use assume;

macro_rules! drop_lifetime {
    ($e: expr) => {
        {
            fn check<T>(_: &mut T){}
            check($e);
            NonNull::from($e).as_mut()
        }
    };
}
pub(crate) use drop_lifetime;

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

type Level0Block<Conf> = Block<
    <Conf as Config>::Level0BitBlock, 
    <Conf as Config>::Level1BlockIndex, 
    <Conf as Config>::Level0BlockIndices
>;
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
/// is [Level0BitBlock]::size() * [Level1BitBlock]::size() * [DataBitBlock]::size().
///
/// Only last level contains blocks of actual data. Empty(skipped) data blocks
/// are not allocated.
///
/// Structure optimized for intersection speed. 
/// _(Other inter-bitset operations are in fact fast too - but intersection has lowest algorithmic complexity.)_
/// Insert/remove/contains is fast O(1) too.
/// 
/// [Level0BitBlock]: crate::config::Config::Level0BitBlock
/// [Level1BitBlock]: crate::config::Config::Level1BitBlock
/// [DataBitBlock]: crate::config::Config::DataBitBlock
//
// # Implementation details
//
// At level1 and data level allocated one empty block at index 0.
// With this hierarchy traverse in deep is completely branchless.   
pub struct BitSet<Conf: Config>{
    level0: Level0Block<Conf>,
    level1: Level1<Conf>,
    data  : LevelData<Conf>,
}

impl<Conf: Config> Default for BitSet<Conf> {
    #[inline]
    fn default() -> Self{
        Self{
            level0: Block::empty(),
            level1: Level::new(vec![Block::empty()]),
            data  : Level::new(vec![Block::empty()]),
        }
    }
}

impl<Conf: Config> Clone for BitSet<Conf> {
    fn clone(&self) -> Self {
        Self{
            level0: self.level0.clone(),
            level1: self.level1.clone(),
            data  : self.data.clone(),
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
    
    /// Max usize, [BitSet] with this `Config` can hold.
    /// 
    /// [BitSet]: crate::BitSet
    #[inline]
    pub const fn max_capacity() -> usize {
        // We occupy one block for "empty" at each level, except root.
        max_addressable_index::<Conf>()
            - (1 << Conf::Level1BitBlock::SIZE_POT_EXPONENT)
            - (1 << Conf::DataBitBlock::SIZE_POT_EXPONENT)
    }    
    
    #[inline]
    fn is_in_range(index: usize) -> bool{
        index < Self::max_capacity()
    }
    
    #[inline]
    fn get_block_indices(&self, level0_index: usize, level1_index: usize)
        -> (usize/*level1_block_index*/, usize/*data_block_index*/)
    {
        let level1_block_index = unsafe{
            self.level0.block_indices().get_unchecked(level0_index)
        }.as_usize();

        // 2. Level1
        let data_block_index = unsafe{
            let level1_block = self.level1.blocks().get_unchecked(level1_block_index);
            level1_block.block_indices().get_unchecked(level1_index)
        }.as_usize();
        
        (level1_block_index, data_block_index)
    }
    
    //TODO: try use Self::level1 instead of self?
    //      or move function to level1?      
    #[inline]
    pub(crate) unsafe fn get_or_insert_level1block(
        &mut self, 
        in_block_level0_index: usize,
    ) -> (usize/*level1_block_index*/, &mut Level1Block<Conf>) {
        let level1_block_index = unsafe{
            self.level0.get_or_insert(in_block_level0_index, ||{
                let block_index = self.level1.insert_empty_block();
                Conf::Level1BlockIndex::from_usize(block_index)
            })
        }.as_usize();  
        
        let level1_block = self.level1.blocks_mut().get_unchecked_mut(level1_block_index);
        (level1_block_index, level1_block)
    }
    
    //TODO: try use Self::data instead of self
    #[inline]
    pub(crate) unsafe fn get_or_insert_datablock(
        &mut self,
        level1_block: &mut Level1Block<Conf>,
        in_block_level1_index: usize
    ) -> (usize/*data_block_index*/, &mut LevelDataBlock<Conf>) {
        let data_block_index =          
            level1_block.get_or_insert(in_block_level1_index, ||{
                let block_index = self.data.insert_empty_block();
                Conf::DataBlockIndex::from_usize(block_index)
            }).as_usize();

        // 3. Data level
        let data_block = self.data.blocks_mut().get_unchecked_mut(data_block_index);

        (data_block_index, data_block)
    }
    
    #[inline]
    pub(crate) unsafe fn insert_impl(&mut self, index: usize) -> (
        usize/*in_block_level0_index*/,
        usize/*level1_block_index*/, &mut Level1Block<Conf>,    usize/*in_block_level1_index*/,
        usize/*data_block_index*/,   &mut LevelDataBlock<Conf>, usize/*in_block_data_index*/,
        u64/*mutated_block_primitive*/
    ) {
        let (in_block_level0_index, in_block_level1_index, in_block_data_index) = level_indices::<Conf>(index);
        
        let (level1_block_index, level1_block) = self.get_or_insert_level1block(in_block_level0_index);
        let level1_block = drop_lifetime!(level1_block);
        let (data_block_index, data_block) = self.get_or_insert_datablock(level1_block, in_block_level1_index);
        let (_, primitive) = data_block.mask_mut().set_bit::<true>(in_block_data_index);
        
        (
            in_block_level0_index,
            level1_block_index, level1_block, in_block_level1_index,
            data_block_index, data_block, in_block_data_index, 
            primitive
        )
    }    
    
    /// # Safety
    ///
    /// Will panic, if `index` is out of range.
    pub fn insert(&mut self, index: usize) {
        assert!(Self::is_in_range(index), "index out of range!");
        unsafe{ self.insert_impl(index); }
    }
    
    #[inline]
    pub(crate) unsafe fn remove_impl(
        &mut self,
        level0_index: usize,
        level1_index: usize,
        data_index: usize,
        level1_block_index: usize,
        data_block_index: usize,
    ) -> bool{
        // 2. Get Data block and set bit
        let data_block = self.data.blocks_mut().get_unchecked_mut(data_block_index);
        let (existed, primitive) = data_block.remove(data_index);
        
        // 3. Remove free blocks
        if primitive == 0 && data_block.is_empty() {
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
        
        existed    
    }

    /// Returns false if index is invalid/not in bitset.
    pub fn remove(&mut self, index: usize) -> bool {
        if !Self::is_in_range(index){
            return false;
        }

        // 1. Resolve indices
        let (level0_index, level1_index, data_index) = level_indices::<Conf>(index);
        let (level1_block_index, data_block_index) = self.get_block_indices(level0_index, level1_index);
        if data_block_index == 0{
            return false;
        }

        unsafe{
            self.remove_impl(
                level0_index, level1_index, data_index,
                level1_block_index, data_block_index
            )
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


/*fn from_bitset_interface<Conf, B>() -> BitSet<Conf>
where
    Conf: Config, 
    B: BitSetInterface<Conf = Conf>
{
    todo!()    
} */

/*impl<Conf: Config, B: BitSetInterface<Conf = Conf>> From<B> for BitSet<Conf> {
    fn from(bitset: B) -> Self {
        todo!()
        
        /*if B::TRUSTED_HIERARCHY {
            // copy as is
            let level0_mask = bitset.level0_mask();
            let mut level0_indices: Conf::Level0BlockIndices = unsafe{
                MaybeUninit::zeroed().assume_init()
            };
            
            level0_mask.traverse_bits(|index|{
                level0_indices.get_unchec
            });
            
            
            let mut level0 = Level0Block::default();
            *level0.mask_mut() = 
            
            let this = Self{
                
            }
        }
        Self::from_iter(value.into_iter())*/
    }
}*/

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
        let level1_block_index = self.level0.block_indices().get_unchecked(level0_index).as_usize();
        let level1_block = self.level1.blocks().get_unchecked(level1_block_index);
        *level1_block.mask()
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize) -> Conf::DataBitBlock {
        let level1_block_index = self.level0.block_indices().get_unchecked(level0_index).as_usize();
        let level1_block = self.level1.blocks().get_unchecked(level1_block_index);

        let data_block_index = level1_block.block_indices().get_unchecked(level1_index).as_usize();
        let data_block = self.data.blocks().get_unchecked(data_block_index);
        *data_block.mask()
    }
}

impl<Conf: Config> LevelMasksIterExt for BitSet<Conf>{
    /// Points to elements in heap. Guaranteed to be stable.
    /// This is just plain pointers with null in default:
    /// `(*const LevelDataBlock<Conf>, *const Level1Block<Conf>)`
    type Level1BlockData = (
        Option<NonNull<LevelDataBlock<Conf>>>,  /* data array pointer */
        Option<NonNull<Level1Block<Conf>>>      /* block pointer */
    );

    type IterState = ();
    fn make_iter_state(&self) -> Self::IterState { () }
    unsafe fn drop_iter_state(&self, _: &mut ManuallyDrop<Self::IterState>) {}

    #[inline]
    unsafe fn init_level1_block_data(
        &self,
        _: &mut Self::IterState,
        level1_block_data: &mut MaybeUninit<Self::Level1BlockData>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool){
        let level1_block_index = self.level0.block_indices().get_unchecked(level0_index);

        // TODO: This can point to static empty block, if level1_block_index invalid.
        //       But looks like this way it is a tiny bit faster.

        let level1_block = self.level1.blocks().get_unchecked(level1_block_index.as_usize());
        level1_block_data.write(
            (
                Some(NonNull::new_unchecked(self.data.blocks().as_ptr() as *mut _)),
                Some(NonNull::from(level1_block))
            )
        );
        (*level1_block.mask(), !level1_block_index.is_zero())
    }

    #[inline]
    unsafe fn data_mask_from_block_data(
        level1_blocks: &Self::Level1BlockData, level1_index: usize
    ) -> Conf::DataBitBlock {
        let array_ptr = level1_blocks.0.unwrap_unchecked().as_ptr().cast_const();
        let level1_block = level1_blocks.1.unwrap_unchecked().as_ref();

        let data_block_index = level1_block.block_indices().get_unchecked(level1_index);
        let data_block = &*array_ptr.add(data_block_index.as_usize());
        *data_block.mask()
    }
}

internals::impl_bitset!(impl<Conf> for ref BitSet<Conf> where Conf: Config);


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
            bit_block_iter: self.bit_block.clone().into_bits_iter()
        }
    }
    
    /// Calculate elements count in DataBlock.
    /// 
    /// On most platforms, this should be faster then manually traversing DataBlock
    /// and counting elements. It use hardware accelerated "popcnt",
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
            bit_block_iter: self.bit_block.into_bits_iter()
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