//! Configurations for [BitSet].
//!
//! Increasing block size will increase max index [BitSet] can hold.
//! Decreasing block size will lower memory footprint.
//!
//! For your task, you can make specialized config. For example, if you're
//! not limited by MAX index, and know that your indices will be dense,
//! you can try 64/64/256 bit levels.
//!
//! [BitSet]: crate::BitSet

use std::marker::PhantomData;
use crate::bit_block::BitBlock;
use crate::{cache, Primitive, PrimitiveArray};
use crate::cache::ReduceCache;
use crate::iter::{CachingBlockIter, CachingIndexIter};

type DefaultCache = cache::FixedCache<32>;
pub(crate) type DefaultBlockIterator<T> = CachingBlockIter<T>;
pub(crate) type DefaultIndexIterator<T> = CachingIndexIter<T>;

/// [BitSet] configuration
/// 
/// [BitSet]: crate::BitSet
pub trait Config: 'static {
// Level 0
    /// BitBlock used as bitmask for level 0.
    type Level0BitBlock: BitBlock;

    /// Contiguous container, used as indirection storage for level 0.
    ///
    /// Must be big enough to accommodate at least [Level0BitBlock]::size().  
    /// Must be `[Self::Level1BlockIndex; 1 << Level0BitBlock::SIZE_POT_EXPONENT]`
    ///
    /// [Level0BitBlock]: Self::Level0BitBlock
    type Level0BlockIndices: PrimitiveArray<Item=Self::Level1BlockIndex>;

// Level 1
// There can be maximum [Level0BitBlock]::size() level1 blocks

    /// BitBlock used as bitmask for level 1 block.
    type Level1BitBlock: BitBlock;

    // TODO: try to remove
    /// Index type, used for indirection from level0 to level1.
    ///
    /// Should be able to store [Level0BitBlock]::size() integer.
    /// 
    /// [Level0BitBlock]: Self::Level0BitBlock
    type Level1BlockIndex: Primitive;

    /// Contiguous container, used as indirection storage for level 1 block.
    ///
    /// Must be big enough to accommodate at least [Level1BitBlock]::size().  
    /// Must be `[Self::DataBlockIndex; 1 << Level1BitBlock::SIZE_POT_EXPONENT]`
    ///
    /// [Level1BitBlock]: Self::Level1BitBlock
    type Level1BlockIndices: PrimitiveArray<Item=Self::DataBlockIndex>;

// Level data
// There can be maximum [Level0BitBlock]::SIZE * [Level1BitBlock]::SIZE data level blocks

    /// BitBlock used as bitmask for data level block.
    type DataBitBlock: BitBlock;

    /// Index type, used for indirection from level1 to data level.
    ///
    /// Should be able to store [Level0BitBlock]::size() * [Level1BitBlock]::size() integer.
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

#[inline]
pub(crate) const fn max_addressable_index<Conf: Config>() -> usize {
    (1 << Conf::Level0BitBlock::SIZE_POT_EXPONENT)
        * (1 << Conf::Level1BitBlock::SIZE_POT_EXPONENT)
        * (1 << Conf::DataBitBlock::SIZE_POT_EXPONENT)
}

// TODO: rename to SmallConfig?
/// [SmallBitSet] configuration.
/// 
/// Try to keep level1 block small. Remember that [Level1BitBlock] has huge align.
/// Try to keep [Level1MaskU64Populations] + [Level1SmallBlockIndices] size within 
/// SIMD align.
pub trait ConfigSmall: Config {
    type Level1SmallBlockIndices : PrimitiveArray<
        Item = <<Self as Config>::Level1BlockIndices as PrimitiveArray>::Item
    >;
    
    /// mask's bit-population at the start of each u64 block.
    /// Should be [u8; Self::Mask::size()/64].
    /// 
    /// P.S. Should be deductible from Mask, but the RUST...  
    type Level1MaskU64Populations: PrimitiveArray<Item=u8>;
}

/// MAX = 262_144
#[derive(Default)]
pub struct _64bit<DefaultCache: ReduceCache = self::DefaultCache>(PhantomData<DefaultCache>);
impl<DefaultCache: ReduceCache> Config for _64bit<DefaultCache> {
    type Level0BitBlock = u64;
    type Level0BlockIndices = [u8; 64];

    type Level1BitBlock = u64;
    type Level1BlockIndex = u8;
    type Level1BlockIndices = [u16; 64];

    type DataBitBlock = u64;
    type DataBlockIndex = u16;

    type DefaultCache = DefaultCache;
}
impl<DefaultCache: ReduceCache> ConfigSmall for _64bit<DefaultCache> {
    type Level1SmallBlockIndices  = [u16;7];
    type Level1MaskU64Populations = [u8;1];
}

/// MAX = 2_097_152
#[cfg(feature = "simd")]
#[cfg_attr(docsrs, doc(cfg(feature = "simd")))]
#[derive(Default)]
pub struct _128bit<DefaultCache: ReduceCache = self::DefaultCache>(PhantomData<DefaultCache>);
#[cfg(feature = "simd")]
#[cfg_attr(docsrs, doc(cfg(feature = "simd")))]
impl<DefaultCache: ReduceCache> Config for _128bit<DefaultCache> {
    type Level0BitBlock = wide::u64x2;
    type Level0BlockIndices = [u8; 128];

    type Level1BitBlock = wide::u64x2;
    type Level1BlockIndex = u8;
    type Level1BlockIndices = [u16; 128];

    type DataBitBlock = wide::u64x2;
    type DataBlockIndex = u16;

    type DefaultCache = DefaultCache;
}
#[cfg(feature = "simd")]
#[cfg_attr(docsrs, doc(cfg(feature = "simd")))]
impl<DefaultCache: ReduceCache> ConfigSmall for _128bit<DefaultCache> {
    type Level1SmallBlockIndices  = [u16;7];
    type Level1MaskU64Populations = [u8;2];
}

/// MAX = 16_777_216 
#[cfg(feature = "simd")]
#[cfg_attr(docsrs, doc(cfg(feature = "simd")))]
#[derive(Default)]
pub struct _256bit<DefaultCache: ReduceCache = self::DefaultCache>(PhantomData<DefaultCache>);
#[cfg(feature = "simd")]
#[cfg_attr(docsrs, doc(cfg(feature = "simd")))]
impl<DefaultCache: ReduceCache> Config for _256bit<DefaultCache> {
    type Level0BitBlock = wide::u64x4;
    type Level0BlockIndices = [u8; 256];

    type Level1BitBlock = wide::u64x4;
    type Level1BlockIndex = u8;
    type Level1BlockIndices = [u16; 256];

    type DataBitBlock = wide::u64x4;
    type DataBlockIndex = u16;

    type DefaultCache = DefaultCache;
}
#[cfg(feature = "simd")]
#[cfg_attr(docsrs, doc(cfg(feature = "simd")))]
impl<DefaultCache: ReduceCache> ConfigSmall for _256bit<DefaultCache> {
    type Level1SmallBlockIndices  = [u16;14];
    type Level1MaskU64Populations = [u8;4];
}