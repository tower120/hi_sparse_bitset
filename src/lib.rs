#![cfg_attr(miri, feature(alloc_layout_extra) )]
#![cfg_attr(docsrs, feature(doc_cfg))]
//! Hierarchical sparse bitset. 
//! 
//! Memory consumption does not depend on max index inserted.
//! 
//! ```text
//! Level0       128bit SIMD                                             
//!               [u8;128]                                               
//!                                     SmallBitSet                      
//!             ┌           ┐  │  ┌                      ┐               
//! Level1  Vec │128bit SIMD│  ┃  │      128bit SIMD     │               
//!             │ [u16;128] │  ┃  │[u16;7]/Box<[u16;128]>│               
//!             └           ┘  │  └                      ┘               
//!             ┌           ┐                                            
//! Data    Vec │128bit SIMD│                                            
//!             └           ┘                                            
//! ────────────────────────────────────────────────────                 
//!                        1 0 1   ...   1  ◀══ bit-mask                 
//! Level0                 □ Ø □         □  ◀══ index-pointers           
//!                      ┗━│━━━│━━━━━━━━━│┛                              
//!                     ╭──╯   ╰──────╮  ╰───────────────────╮           
//!           1 0 0 1   ▽             ▽   1                  ▽           
//! Level1    □ Ø Ø □  ...           ...  □  ...                         
//!         ┗━│━━━━━│━━━━━━━┛     ┗━━━━━━━│━━━━━━┛    ┗━━━━━━━━━━━━━━┛   
//!           ╰──╮  ╰─────────────────╮   ╰───────────────╮              
//!              ▽                    ▽                   ▽              
//! Data     1 0 0 0 1 ...       0 0 1 1 0 ...       0 1 0 1 0 ...    ...
//!        ┗━━━━━━━━━━━━━━━┛   ┗━━━━━━━━━━━━━━━┛   ┗━━━━━━━━━━━━━━━┛
//! ```
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
//! # SmallBitset
//! 
//! [SmallBitSet] is like [BitSet], but have **significantly** lower memory footprint
//! on sparse sets. If not some performance overhead - that would be the one and
//! only container in this lib. 
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
//! # CPU extensions
//! 
//! Library uses `popcnt`/`count_ones` and `tzcnt`/`trailing_zeros` heavily.
//! Make sure you compile with hardware support of these 
//! (on x86: `target-feature=+popcnt,+bmi1`).
//! 
//! ## SIMD
//! 
//! 128 and 256 bit configurations use SIMD, powered by [wide]. Make sure you compile with simd support
//! enabled (on x86: `sse2` for _128bit, `avx` for _256bit) to achieve best performance.
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
mod compact_block;
mod raw;
mod small_bitset;
mod primitive_array;
mod bitset;

pub use bitset_interface::{BitSetBase, BitSetInterface};
pub use apply::Apply;
pub use reduce::Reduce;
pub use bit_block::BitBlock;
pub use bitset::BitSet;
pub use small_bitset::SmallBitSet;

use primitive::Primitive;
use primitive_array::PrimitiveArray;
use std::ops::ControlFlow;
use config::Config;
use level::IBlock;
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