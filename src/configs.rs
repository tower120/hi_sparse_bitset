//! The smaller the block size - the lower `HiSparseBitset` memory footprint.
//!
//! For your task, you can make specialized config. For example, if you're
//! not limited by MAX index, and know that your indices will be dense,
//! you can try 64/64/256 bit levels.

use crate::cache::FixedCache;
use crate::IConfig;
use crate::iter::CachingBlockIter;
use crate::virtual_bitset::{LevelMasks, LevelMasksExt3};

/// MAX = 262_144
#[derive(Default)]
pub struct _64bit;

// TODO: Use SimpleBlockIter

impl IConfig for _64bit {
    type Level0BitBlock = u64;
    type Level0BlockIndices = [u8; 64];

    type Level1BitBlock = u64;
    type Level1BlockIndex = u8;
    type Level1BlockIndices = [u16; 64];

    type DataBitBlock = u64;
    type DataBlockIndex = u16;

    type DefaultBlockIterator<T: LevelMasksExt3> = CachingBlockIter<T>;

    // TODO: refactor this somehow?
    type DefaultCache = FixedCache<32>;
}

/// MAX = 2_097_152
#[cfg(feature = "simd")]
#[derive(Default)]
pub struct _128bit;

#[cfg(feature = "simd")]
impl IConfig for _128bit {
    type Level0BitBlock = wide::u64x2;
    type Level0BlockIndices = [u8; 128];

    type Level1BitBlock = wide::u64x2;
    type Level1BlockIndex = u8;
    type Level1BlockIndices = [u16; 128];

    type DataBitBlock = wide::u64x2;
    type DataBlockIndex = u16;

    type DefaultBlockIterator<T: LevelMasksExt3> = CachingBlockIter<T>;
    type DefaultCache = FixedCache<32>;
}

// TODO: simd_256