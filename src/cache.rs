//! Cache used by [CachingBlockIter] for [reduce] operations.
//!
//! # Memory footprint
//!
//! Cache for one [BitSet] costs 2 pointers.
//!
//! [reduce]: crate::reduce()

use crate::binary_op::BinaryOp;
use crate::bitset_interface::{BitSetBase, LevelMasksExt};
use crate::reduce::{DynamicCacheImpl, FixedCacheImpl, NonCachedImpl, ReduceCacheImpl};

/// Cache is not used.
///
/// This also discards cache usage for all underlying [reduce] operations.
/// Cache still can be applied on top of NoCache operation.
///
/// # Example
///
/// TODO
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
#[derive(Default, Copy, Clone)]
pub struct DynamicCache;

pub trait ReduceCache: Default + 'static{
    /// usize::MAX - if unlimited.
    const MAX_LEN: usize;
    type Impl<Op, S>
        : ReduceCacheImpl<
            Sets = S,
            Config = <S::Item as BitSetBase>::Config
        >
    where
        Op: BinaryOp,
        S: Iterator + Clone,
        S::Item: LevelMasksExt;
}

impl ReduceCache for NoCache{
    const MAX_LEN: usize = usize::MAX;
    type Impl<Op, S> = NonCachedImpl<Op, S>
    where
        Op: BinaryOp,
        S: Iterator + Clone,
        S::Item: LevelMasksExt;
}

impl<const N: usize> ReduceCache for FixedCache<N>{
    const MAX_LEN: usize = N;
    type Impl<Op, S> = FixedCacheImpl<Op, S, N>
    where
        Op: BinaryOp,
        S: Iterator + Clone,
        S::Item: LevelMasksExt;
}

impl ReduceCache for DynamicCache{
    const MAX_LEN: usize = usize::MAX;
    type Impl<Op, S> = DynamicCacheImpl<Op, S>
    where
        Op: BinaryOp,
        S: Iterator + Clone,
        S::Item: LevelMasksExt;
}