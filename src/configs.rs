//! The smaller the block size - the lower `HiSparseBitset` memory footprint.
//!
//! For your task, you can make specialized config. For example, if you're
//! not limited by MAX index, and know that your indices will be dense,
//! you can try 64/64/256 bit levels.

use crate::cache;
use crate::IConfig;
use crate::iter::CachingBlockIter;
use crate::bitset_interface::{LevelMasks, LevelMasksExt};

type DefaultCache = cache::FixedCache<32>;

// TODO: rename to with_cache?
pub mod base{
    use std::marker::PhantomData;
    use crate::reduce::ReduceCacheImplBuilder;
    use crate::IConfig;
    use crate::iter::CachingBlockIter;
    use crate::bitset_interface::LevelMasksExt;

    #[derive(Default)]
    pub struct _64bit<DefaultCache: ReduceCacheImplBuilder>(PhantomData<DefaultCache>);
    impl<DefaultCache: ReduceCacheImplBuilder> IConfig for _64bit<DefaultCache> {
        type Level0BitBlock = u64;
        type Level0BlockIndices = [u8; 64];

        type Level1BitBlock = u64;
        type Level1BlockIndex = u8;
        type Level1BlockIndices = [u16; 64];

        type DataBitBlock = u64;
        type DataBlockIndex = u16;

        type DefaultBlockIterator<T: LevelMasksExt> = CachingBlockIter<T>;

        // TODO: refactor this somehow?
        type DefaultCache = DefaultCache;
    }

    #[cfg(feature = "simd")]
    #[derive(Default)]
    pub struct _128bit<DefaultCache: ReduceCacheImplBuilder>(PhantomData<DefaultCache>);
    #[cfg(feature = "simd")]
    impl<DefaultCache: ReduceCacheImplBuilder> IConfig for _128bit<DefaultCache> {
        type Level0BitBlock = wide::u64x2;
        type Level0BlockIndices = [u8; 128];

        type Level1BitBlock = wide::u64x2;
        type Level1BlockIndex = u8;
        type Level1BlockIndices = [u16; 128];

        type DataBitBlock = wide::u64x2;
        type DataBlockIndex = u16;

        type DefaultBlockIterator<T: LevelMasksExt> = CachingBlockIter<T>;
        type DefaultCache = DefaultCache;
    }
}

/// MAX = 262_144
pub type _64bit = base::_64bit<DefaultCache>;

/// MAX = 2_097_152
#[cfg(feature = "simd")]
pub type _128bit = base::_128bit<DefaultCache>;

// TODO: simd_256