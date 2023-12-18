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

<picture>
  <source srcset="https://github.com/tower120/hi_sparse_bitset/raw/main/doc/hisparsebitset-dark-50.png" media="(prefers-color-scheme: dark)">
  <source srcset="https://github.com/tower120/hi_sparse_bitset/raw/main/doc/hisparsebitset-50.png" media="(prefers-color-scheme: light)">
  <img src="https://github.com/tower120/hi_sparse_bitset/raw/main/doc/hisparsebitset-bg-white-50.png">
</picture>

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
hibitset. See benchmarks.

## Against `hibitset`

Despite the fact that `hi_sparse_bitset` have layer of indirection for accessing
each level, it is faster (sometimes significantly) then `hibitset` for all operations.

On top of that, it is also **algorithmically** faster than `hibitset` in 
non-intersection inter-bitset operations due to caching iterator, which
can skip bitsets with empty level1 blocks. 

## Against `roaring`

`roaring` is a hybrid bitset, that use sorted array of bitblocks for set with large integers,
and big fixed-sized bitset for a small ones.
Let's consider case for intersecting `roaring` sets, that contain large integers.
In order to find intersection, it binary search for bitblocks with the same start index,
then intersect each bitblock. Operation of binary searching matching bitblock 
is algorithmically more complex O(log N), then directly traversing intersected 
bitblock in hierarchy, which is close to O(1) for each resulted bitblock.
Plus, hierarchical bitset discard groups of non-intersected blocks
early, due to its tree-like nature.

# DataBlock operations

In order to speed up things even more, you can work directly with
`DataBlock`s. `DataBlock`s - is a bit-blocks (relatively small in size), 
which you can store and iterate latter.

_In future versions, you can also insert DataBlocks into BitSet._

# Reduce on iterator of bitsets

In addition to "the usual" bitset-to-bitset(binary) operations,
you can apply operation to iterator of bitsets (reduce/fold).
In this way, you not only apply operation to the arbitrary
number of bitsets, but also have the same result type,
for any bitsets count. Which allows to have nested reduce
operations.

# Ordered/sorted

Iteration always return sorted sequences.

# Suspend-resume iterator with cursor

Iterators of `BitSetInterface` (any kind of bitset) can return cursor, 
and can rewind to cursor. Cursor is like integer index in `Vec`.
Which means, that you can use it even if container was mutated.

## Multi-session iteration

With cursor you can suspend and later resume your iteration 
session. For example, you can create an intersection between several bitsets, iterate it
to a certain point, and obtain an iterator cursor. Then, later,
you can make an intersection between the same bitsets (but possibly in different state),
and resume iteration from the last point you stopped, using cursor.

## Thread safe multi-session iteration

You can use "multi-session iteration" in multithreaded env too.
_(By wrapping bitsets in Mutex(es))_

### Invariant intersection

If intersection of bitsets _(or any other operation)_ does not change with possible bitset mutations - you're guaranteed to correctly traverse all of its elements.

### Bitsets mutations narrows intersection/union

If in intersection, only `remove` operation mutates bitsets - this guarantees that you will traverse all elements that still exists. (thou this elements can be different at different point in time)

### Speculative iteration

For other cases - you're guaranteed to proceed forward, without repeated elements.
_(On each iteration session you'll see initial valid elements + some valid new ones)_
You can use this if you don't need to traverse EXACT intersection. For example, if you
process intersection of the same bitsets over and over in a loop.

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

* `HashSet<usize>` - you should use it only if you work with a relatively small
   set with extremely large numbers. 
   It is orders of magnitude slower for inter-set operations.
   And "just" slower for the rest ones.

*  [roaring](https://crates.io/crates/roaring) - compressed hybrid bitset. 
   Higher algorithmic complexity of operations, but theoretically unlimited range.
   It is still super-linearly faster then pure dense bitsets and hashsets in inter-set
   operations. See [performance section](#against-roaring) for detais.