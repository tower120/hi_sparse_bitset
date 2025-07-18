#![cfg_attr(miri, feature(alloc_layout_extra) )]
#![cfg_attr(docsrs, feature(doc_cfg))]
//! Hierarchical sparse bitset. 
//! 
//! Memory consumption does not depend on max index inserted.
//! 
//! ```text
//! Level0       128bit SIMD                                             
//!               [u8;128]                                               
//!                                                  
//!             ┌           ┐                 
//! Level1  Vec │128bit SIMD│                 
//!             │ [u16;128] │                 
//!             └           ┘                 
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
//! # Laziness and materialization
//! 
//! Use [BitSet::from(impl BitSetInterface)] instead of collecting iterator for
//! materialization into BitSet.
//! 
//! # Cursor
//! 
//! [BitSetInterface] iterators can return [cursor()], pointing to the current iterator position. 
//! You can use [Cursor] to move ANY [BitSetInterface] iterator to it's position with [move_to].
//! 
//! You can also build cursor from index.
//! 
//! [cursor()]: crate::iter::IndexIter::cursor
//! [Cursor]: crate::iter::IndexCursor
//! [move_to]: crate::iter::IndexIter::move_to
//! 
//! # Iterator::for_each
//! 
//! [BitSetInterface] iterators have [for_each] specialization and stable [try_for_each] version - [traverse].
//! For tight loops, traversing is observably faster then iterating.
//! 
//! [for_each]: std::iter::Iterator::for_each
//! [try_for_each]: std::iter::Iterator::try_for_each
//! [traverse]: crate::iter::IndexIter::traverse
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
//! # Serialization/Serde
//! 
//! Enable feature `serde` - for [serde] serialization support.
//! 
//! [serde]: https://crates.io/crates/serde
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
//! If you don't need "wide" configurations, you may disable default feature `simd`.
//!
//! [wide]: https://crates.io/crates/wide

#[cfg(test)]
mod test;

mod primitive;
mod primitive_array;
mod block;
mod level;
mod bit_block;
mod bit_queue;
mod bit_utils;
mod reduce;
mod bitset_interface;
mod apply;
mod raw;
mod derive_raw;
mod bitset;
mod internals;
mod data_block;

pub mod config;
pub mod ops;
pub mod iter;
pub mod cache;

pub use bitset_interface::{BitSetBase, BitSetInterface};
pub use apply::Apply;
pub use reduce::Reduce;
pub use bit_block::BitBlock;
pub use bitset::BitSet;
pub use data_block::{DataBlock, DataBlockIter};

use primitive::Primitive;
use primitive_array::PrimitiveArray;
use config::Config;
use ops::BitSetOp;
use cache::ReduceCache;

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