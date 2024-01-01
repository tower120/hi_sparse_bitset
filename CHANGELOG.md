# Changelog

## 0.4.0
### Changed
-  `BitSetInterface` changed (simplified).
- `BitSetOp` renamed to `BitSetApp`.
- `BinaryOp` renamed to `BitSetOp`.
- `binary_op` module renamed to `ops`.

### Added
- `BitSet`, `BitSetOp`, `Reduce` now duplicate part of `BitSetInterface` in 
order to prevent the need of `BitSetInterface` import.
- `CachingIndexIter` now have former `IndexIterator` functionality.
- `CachingBlockIter` now have former `BlockIterator` functionality.

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