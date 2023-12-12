# Changelog

## 0.3.0
### Changed 
- General minor performance improvement (removed index check in bit-manipulation).
- `BitSetInterface`'s `IntoIterator` now implements `IndexIter`.
- `BlockIterCursor` renamed to `BlockCursor`.
- `IndexIterCursor` renamed to `IndexCursor`.
- both cursors now <= 64bit in size.

### Added
- `BlockCursor` now implements `Copy`.
- `IndexCursor` now implements `Copy`.
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

- `BitSet::is_empty`.
- All `BitSetInterface`s now implements `Eq`.
- `BitSet` (without &) now implements `op`s too.

## 0.2.0
### Changed
- `IConfig` renamed to `Config`.
- `max_range()` moved to `Config`.

### Added
- `BitBlock` trait for implementing custom `BitSet` bitblock.
- `_256bit` `Config` added.

## 0.1.0

Initial version