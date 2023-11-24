//! Configurations for [BitSet].
//!
//! The smaller the block size - the lower `HiSparseBitset` memory footprint.
//!
//! For your task, you can make specialized config. For example, if you're
//! not limited by MAX index, and know that your indices will be dense,
//! you can try 64/64/256 bit levels.
//!
//! [BitSet]: crate::BitSet

use crate::bit_block::BitBlock;
use crate::{cache, Primitive};
use crate::cache::ReduceCache;
use crate::iter::CachingBlockIter;

type DefaultCache = cache::FixedCache<32>;
pub(crate) type DefaultBlockIterator<T> = CachingBlockIter<T>;

/// [BitSet] configuration
/// 
/// [BitSet]: crate::BitSet
pub trait IConfig: 'static {
// Level 0
    /// BitBlock used as bitmask for level 0.
    type Level0BitBlock: BitBlock + Default;

    /// Contiguous container, used as indirection storage for level 0.
    ///
    /// Must be big enough to accommodate at least [Level0BitBlock]::SIZE.  
    /// Must be `[Self::Level1BlockIndex; 1 << Level0BitBlock::SIZE_POT_EXPONENT]`
    ///
    /// [Level0BitBlock]: Self::Level0BitBlock
    type Level0BlockIndices: AsRef<[Self::Level1BlockIndex]> + AsMut<[Self::Level1BlockIndex]> + Clone;

// Level 1
// There can be maximum [Level0BitBlock]::SIZE level1 blocks

    /// BitBlock used as bitmask for level 1 block.
    type Level1BitBlock: BitBlock + Default;

    /// Index type, used for indirection from level0 to level1.
    ///
    /// Should be able to store [Level0BitBlock]::SIZE integer.
    /// 
    /// [Level0BitBlock]: Self::Level0BitBlock
    type Level1BlockIndex: Primitive;

    /// Contiguous container, used as indirection storage for level 1 block.
    ///
    /// Must be big enough to accommodate at least [Level1BitBlock]::SIZE.  
    /// Must be `[Self::DataBlockIndex; 1 << Level1BitBlock::SIZE_POT_EXPONENT]`
    ///
    /// [Level1BitBlock]: Self::Level1BitBlock
    type Level1BlockIndices: AsRef<[Self::DataBlockIndex]> + AsMut<[Self::DataBlockIndex]> + Clone;

// Level data
// There can be maximum [Level0BitBlock]::SIZE * [Level1BitBlock]::SIZE data level blocks

    /// BitBlock used as bitmask for data level block.
    type DataBitBlock: BitBlock + Default;

    /// Index type, used for indirection from level1 to data level.
    ///
    /// Should be able to store [Level0BitBlock]::SIZE * [Level1BitBlock]::SIZE integer.
    ///
    /// [Level0BitBlock]: Self::Level0BitBlock
    /// [Level1BitBlock]: Self::Level1BitBlock
    type DataBlockIndex: Primitive;

// Other

    /// Cache used be [reduce()].
    /// 
    /// [reduce()]: crate::reduce()
    type DefaultCache: ReduceCache;
}

/// Specify the default cache type.
pub mod with_cache{
    use std::marker::PhantomData;
    use crate::cache::ReduceCache;
    use crate::config::IConfig;

    /// MAX = 262_144
    #[derive(Default)]
    pub struct _64bit<DefaultCache: ReduceCache>(PhantomData<DefaultCache>);
    impl<DefaultCache: ReduceCache> IConfig for _64bit<DefaultCache> {
        type Level0BitBlock = u64;
        type Level0BlockIndices = [u8; 64];

        type Level1BitBlock = u64;
        type Level1BlockIndex = u8;
        type Level1BlockIndices = [u16; 64];

        type DataBitBlock = u64;
        type DataBlockIndex = u16;

        type DefaultCache = DefaultCache;
    }

    /// MAX = 2_097_152
    #[cfg(feature = "simd")]
    #[derive(Default)]
    pub struct _128bit<DefaultCache: ReduceCache>(PhantomData<DefaultCache>);
    #[cfg(feature = "simd")]
    impl<DefaultCache: ReduceCache> IConfig for _128bit<DefaultCache> {
        type Level0BitBlock = wide::u64x2;
        type Level0BlockIndices = [u8; 128];

        type Level1BitBlock = wide::u64x2;
        type Level1BlockIndex = u8;
        type Level1BlockIndices = [u16; 128];

        type DataBitBlock = wide::u64x2;
        type DataBlockIndex = u16;

        type DefaultCache = DefaultCache;
    }
}

/// MAX = 262_144
pub type _64bit = with_cache::_64bit<DefaultCache>;

/// MAX = 2_097_152
pub type _128bit = with_cache::_128bit<DefaultCache>;

// TODO: simd_256