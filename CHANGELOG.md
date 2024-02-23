# Changelog

## 0.5.1
### Fix
- Intersection operation `ops::And` does **not** produce `TRUSTED_HIERARCHY`.

## 0.5.0
### Fix
- `NoCache` reduce version was unsound for iterators which had to be dropped. 

### Optimization
- On each level, instead of empty block indices Vec, an intrusive single-linked list is now used.
  This completely eliminates this kind of memory overhead. Previously, if you would fill `_256bit` bitset,
  and then clear it - you would end up with additional 132Kb memory overhead from the list of free blocks.
  Considering that preallocated bitblocks themselves took 2Mb, this saves more than 5% of memory.
- Minor `BitSet::remove()` optimization. 

### Changed
- `BitSetInterface` now have default implementation.
- `BitSet` no longer implements `BitSetInterface`. 
  But `&BitSet` still does. This prevents accidental sending container by value.
- `config::with_cache::*` moved to `config::*` with additional default generic argument.
- `crate::bit_queue` moved to `internals::bit_queue`.
- `crate::Primitive` moved to `internals::Primitive`.
- `Config` bitblocks no longer require `Default`.

### Added
- `BitBlock::as_array()`.
- `BitBlock::as_array_mut()`.
- Some `BitBlock` methods  now have default implementations.
- `BitSetOp::HIERARCHY_OPERANDS_CONTAIN_RESULT` marker, for intersection-like 
  optimization in user-defined operations.
- Machinery, which allows to implement custom bitsets. Enabled with `impl` flag.
- `internals` module, with implementation details that end user can use for deep customization.

### Removed
- `num_traits` dependency removed.

## 0.4.0
### Fix
- `Eq` did not work correctly between !`TrustedHierarchy` bitsets.

### Changed
-  `BitSetInterface` changed (simplified).
- `BitSetOp` renamed to `Apply`.
- `BinaryOp` renamed to `BitSetOp`.
- `binary_op` module renamed to `ops`.
- All former `binary_op` operations renamed.

### Added
- `BitSet`, `BitSetOp`, `Reduce` now duplicate part of `BitSetInterface` in 
order to prevent the need of `BitSetInterface` import.
- `CachingIndexIter` now have former `IndexIterator` functionality.
- `CachingBlockIter` now have former `BlockIterator` functionality.
- `BitSetInterface::is_empty()`.
- `BitSetBase::TRUSTED_HIERARCHY`.

### Removed
- `IndexIterator` removed.
- `BlockIterator` removed.

## 0.3.0
### Fix
- `IndexIter::move_to` to the empty bitset area fix.

### Changed 
- General minor performance improvement (removed index check in bit-manipulation).
- `BitSetInterface`'s `IntoIterator` now implement `IndexIter`.
- `BlockIterCursor` renamed to `BlockCursor`.
- `IndexIterCursor` renamed to `IndexCursor`.
- `BlockCursor` and `IndexCursor` now have `Conf` generic parameter.
- both cursors now <= 64bit in size.
- `BlockIter::as_indices` renamed to `BlockIter::into_indices`. 

### Added
- `BlockCursor` now implements `Copy`.
- `IndexCursor` now implements `Copy`.
- `BlockCursor::start()`.
- `BlockCursor::end()`.
- `BlockCursor::from(usize)`.
- `BlockCursor::from(&DataBlock)`.
- `IndexCursor::start()`.
- `IndexCursor::end()`.
- `IndexCursor::from(usize)`.
- `IndexCursor::from(&DataBlock)`.
- `CachingBlockIter` now implements `Clone`.
- `CachingIndexIter` now implements `Clone`.
- `CachingBlockIter::traverse`.
- `CachingIndexIter::traverse`.
- `CachingBlockIter` specialized `for_each` implementation.
- `CachingIndexIter` specialized `for_each` implementation.
- `DataBlockIter` now implements `Clone`.
- `DataBlockIter::traverse`.
- `DataBlockIter` specialized `for_each` implementation.
- `DataBlock` now implements `Eq`.
- All `BitSetInterface`s now implement `Eq`.
- All `BitSetInterface`s now implement `Debug`.
- `BitSet` (without &) now implements `op`s too.

### Removed
- `IndexIterator::as_blocks()` removed as redundant, the same can be achieved with cursor move. 
  And the very need of moving from index iterator to block iterator is questionable.

## 0.2.0
### Changed
- `IConfig` renamed to `Config`.
- `max_range()` moved to `Config`.

### Added
- `BitBlock` trait for implementing custom `BitSet` bitblock.
- `_256bit` `Config` added.

## 0.1.0

Initial version