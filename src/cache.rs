//! Cache used by [CachingBlockIter] for [reduce] operations.
//!
//! # Memory footprint
//!
//! Cache for one [BitSet] costs 2 pointers.
//!
//! [reduce]: crate::reduce()

/// Cache is not used.
///
/// This also discards cache usage for all underlying virtual sets.
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