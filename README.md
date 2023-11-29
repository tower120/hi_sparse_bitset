[![crates.io](https://img.shields.io/crates/v/hi_sparse_bitset.svg)](https://crates.io/crates/hi_sparse_bitset)
[![license](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue?style=flat-square)](#license)
[![Docs](https://docs.rs/hi_sparse_bitset/badge.svg)](https://docs.rs/hi_sparse_bitset)
[![CI](https://github.com/tower120/hi_sparse_bitset/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/tower120/hi_sparse_bitset/actions/workflows/ci.yml)

Hierarchical sparse bitset. High performance of operations between bitsets (intersection, union, etc.).
Low memory usage.

Think of [hibitset](https://crates.io/crates/hibitset), but with lower memory consumption.
Unlike hibitset - it is actually sparse - it's memory usage does not depend on max index in set.
Only amount of used bitblocks matters (or elements, to put it simply).
And like hibitset, it also utilizes hierarchical acceleration structure to reduce
algorithmic complexity on operations between bitsets.

![](https://github.com/tower120/hi_sparse_bitset/raw/main/doc/hisparsebitset-bg-white-50.png)

# Usage 

```rust
type BitSet = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_128bit>;
let bitset1 = BitSet::from([1,2,3,4]);
let bitset2 = BitSet::from([3,4,5,6]);
let bitset3 = BitSet::from([3,4,7,8]);
let bitset4 = BitSet::from([4,9,10]);
let bitsets = [bitset1, bitset2, bitset3];

// reduce on bitsets iterator
let intersection = reduce(BitAndOp, bitsets.iter()).unwrap();
assert_equal(&intersection, [3,4]);

// operation between different types
let union = intersection | &bitset4;
assert_equal(&union, [3,4,9,10]);

// partially traverse iterator, and save position to cursor.
let mut iter = union.iter();
assert_equal(iter.by_ref().take(2), [3,4]);
let cursor = iter.cursor();

// resume iteration from cursor position
let iter = union.iter().move_to(cursor);
assert_equal(iter, [9,10]);
```

# Memory footprint

Being truly sparse, `hi_sparse_bitset` allocate memory only for bitblocks in use.
`hi_sparse_bitset::BitSet` has tri-level hierarchy, with first and second levels
containing bit-masks and indirection information, and third level - actual bit data.
Currently, whole first level (which is one block itself) and one block from the
second level are always allocated.

Hierarchy-wise memory overhead, for `config::_128bit`:
minimal(initial) = 416 bytes, maximum = 35 Kb.

See doc for more info.

# Performance

It is faster than hashsets and pure bitsets for all inter-bitset operations
and all cases in orders of magnitude. It is even faster than 
hibitset, despite hi_sparse_bitset having additional source of
indirection. See benchmarks.

## Against `hibitset`

Despite the fact that `hi_sparse_bitset` have layer of indirection for accessing
each level, it is faster (sometimes significantly) then `hibitset` for all operations.

On top of that, it is also **algorithmically** faster than `hibitset` on 
all non-intersection operations due to caching iterator, which
can skip bitsets with empty data blocks on pre-data level. 

Technical details:
_Hierarchical structure of both `hibitset` and `hi_sparse_bitset` is most
friendly to intersection operations. Doing, for example, union between bitsets,
require for each level of each bitset to take bitblock and OR them. `hi_sparse_bitset`
cache level1 blocks during iteration (as a form of iteration optimisation, since it access 
blocks through indirection), 
and can skip the empty ones. That excludes bitsets with empty level1 blocks completely 
from participating in data level operation._

# DataBlock operations

In order to speed up things even more, you can work directly with
`DataBlock`s. `DataBlock`s - is a bit-blocks (relatively small in size), 
which you can store and iterate latter.

_In future versions, you can also insert DataBlocks into BitSet._

# Reduce on iterator of bitsets

In addition to "the usual" bitset-to-bitset operations,
you can perform operation between all elements of iterator of bitsets.
This is an important addition, since the result is the same type, regardless 
of bitset count. And of course, you can have reduce on reduce on reduce...

# Ordered/sorted

Iteration always return sorted sequences.

# Suspend-resume iterator with cursor

Iterators of `BitSetInterface` (any kind of bitset) can return cursor, 
and can rewind to cursor. Cursor is like integer index in `Vec`.
Which means, that you can use it even if container was mutated.

## Multi-session iteration

This way you can suspend and later resume your iteration 
session. For example, you can have intersection between several bitsets, iterate it
to some point, and get iterator cursor. Then, later,
you can make intersection between the same bitsets (but possibly in different state),
and resume iteration from the last point you stopped, using cursor.

## Multi-threaded env use-case

In multithreaded env, you can lock your bitsets, read part of intersection into buffer,
unlock, process buffer, repeat until the end.

# Known alternatives

* [hibitset](https://crates.io/crates/hibitset) - hierarchical dense bitset. 
    If you'll insert one index = 16_000_000, it will allocate 2Mb of RAM. 
    It uses 4-level hierarchy, and being dense - does not use indirection.
    This makes it hierarchy overhead smaller, and on intersection operations it SHOULD perform
    better - but it doesn't (probably because of additional level of hierarchy, or some 
    implementation details).

* [bitvec](https://crates.io/crates/bitvec) - pure dense bitset. Plain operations (insert/contains)
    should be reasonably faster (not at magnitude scale).
    Inter-bitset operations - super-linearly slower for the worst case (which is almost always), 
    and have approx. same performance for the best case (when each bitset block used).
    Have no memory overhead per-se, but memory usage depends on max int in bitset, 
    so if you do not need to perform inter-bitset operations,
    and know that your indices are relatively small numbers, or expect bitset to be
    densely populated - this is a good choice.

* `HashSet<usize>` - you should use it only if you work with extremely large numbers. 
   It is orders of magnitude slower for inter-bitset operations.
   And "just" slower for the rest ones.

*  [roaring](https://crates.io/crates/roaring) - compressed bitset. It does not have means of intersecting multiple
   sets at once, only through intermediate bitset (which would be unfair to compare). So you can't directly do the same things in `roaring`.
   As for comparing things that possible (like intersection count). In ideal (against hierarchical bitset) 
   for `roaring` scenario (all elements intersects): on quite sparse bitsets roaring is somewhat faster, on denser - slower. 
   That will vary from actual dataset to dataset. Probably the less the percentage of intersected 
   elements - the bigger `hi_sparse_bitset` performance gains against `roaring`.
   The main selling point of `roaring` against `hi_sparse_bitset` should be the fact that `roaring`, being
   compressed bitset can store MUCH bigger indices in set. _DISCLAIMER: It was not benchmarked head-to-head thoroughly_ 
