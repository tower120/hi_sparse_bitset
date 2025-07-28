//! Cache used by [BlockIter]/[IndexIter] for [reduce] operations.
//!
//! # Memory footprint
//!
//! Cache for one [BitSet] costs 2 pointers.
//!
//! [BitSet]: crate::BitSet
//! [BlockIter]: crate::iter::BlockIter
//! [IndexIter]: crate::iter::IndexIter
//! [reduce]: crate::reduce()

use crate::ops::BitSetOp;
use crate::bitset_interface::{BitSetBase, LevelMasksIterExt};
use crate::reduce::{DynamicCacheImpl, FixedCacheImpl, NonCachedImpl, ReduceCacheImpl};

/// Cache is not used.
///
/// If reduced iterator contains other nested reduce operations - all of them
/// WILL NOT use cache as well.
///
/// # Example 1
///
/// ```
/// # use itertools::assert_equal;
/// # use hi_sparse_bitset::{reduce, reduce_w_cache};
/// # use hi_sparse_bitset::ops::{And, Or};
/// # use hi_sparse_bitset::cache::NoCache;
/// # type BitSet = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_128bit>;
/// let su1 = [BitSet::from([1,2]), BitSet::from([5,6])];
/// let union1 = reduce(Or, su1.iter()).unwrap();
///
/// let su2 = [BitSet::from([1,3]), BitSet::from([4,6])];
/// let union2 = reduce(Or, su2.iter()).unwrap();
///
/// let si = [union1, union2];
/// let intersection = reduce_w_cache(And, si.iter(), NoCache).unwrap();
///
/// // Not only `intersection` will be computed without cache,
/// // but also `union1` and `union2`.
/// assert_equal(intersection, [1,6]);
///
/// ```
/// 
/// # Example 2
///
/// ```
/// # use itertools::assert_equal;
/// # use hi_sparse_bitset::{reduce, reduce_w_cache};
/// # use hi_sparse_bitset::ops::{And, Or};
/// # use hi_sparse_bitset::cache::NoCache;
/// # type BitSet = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_128bit>;
/// let su1 = [BitSet::from([1,2]), BitSet::from([5,6])];
/// let union1 = reduce_w_cache(Or, su1.iter(), NoCache).unwrap();
///
/// let su2 = [BitSet::from([1,3]), BitSet::from([4,6])];
/// let union2 = reduce_w_cache(Or, su2.iter(), NoCache).unwrap();
///
/// let si = [union1, union2];
/// let intersection = reduce(And, si.iter()).unwrap();
///
/// // Only `union1` and `union2` will not use cache, `intersection`
/// // will be computed with cache.
/// assert_equal(intersection, [1,6]);
///
/// ```
/// 
/// [reduce]: crate::reduce()
#[derive(Default, Copy, Clone)]
pub struct NoCache;

/// Cache with fixed capacity.
///
/// This cache is noop to construct.
/// Should be your default choice.
///
/// N.B. Pay attention to stack-mem usage when working with
/// reduce on reduce on reduce ...
#[derive(Default, Copy, Clone)]
pub struct FixedCache<const N:usize>;

/// Dynamically built in-heap cache.
///
/// You want this, when your cache doesn't fit stack.
/// This can happened, when you work with enormously large number of sets,
/// and/or work with deep [reduce] operations. Alternatively, you
/// can use [NoCache].
/// 
/// [reduce]: crate::reduce()
#[derive(Default, Copy, Clone)]
pub struct DynamicCache;

pub trait ReduceCache: Default + 'static{
    /// usize::MAX - if unlimited.
    const MAX_LEN: usize;
    type Impl<Op, S>
        : ReduceCacheImpl<
            Sets = S,
            Conf = <S::Item as BitSetBase>::Conf
        >
    where
        Op: BitSetOp,
        S: Iterator + Clone,
        S::Item: LevelMasksIterExt;
}

impl ReduceCache for NoCache{
    const MAX_LEN: usize = usize::MAX;
    type Impl<Op, S> = NonCachedImpl<Op, S>
    where
        Op: BitSetOp,
        S: Iterator + Clone,
        S::Item: LevelMasksIterExt;
}

impl<const N: usize> ReduceCache for FixedCache<N>{
    const MAX_LEN: usize = N;
    type Impl<Op, S> = FixedCacheImpl<Op, S, N>
    where
        Op: BitSetOp,
        S: Iterator + Clone,
        S::Item: LevelMasksIterExt;
}

impl ReduceCache for DynamicCache{
    const MAX_LEN: usize = usize::MAX;
    type Impl<Op, S> = DynamicCacheImpl<Op, S>
    where
        Op: BitSetOp,
        S: Iterator + Clone,
        S::Item: LevelMasksIterExt;
}