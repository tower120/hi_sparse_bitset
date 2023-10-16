//! The smaller the block size - the lower `HiSparseBitset` memory footprint.

use crate::IConfig;

/// MAX = 262_144
#[derive(Default)]
pub struct u64s;

impl IConfig for u64s {
    type Level0BitBlock = u64;
    type Level0BlockIndices = [u8; 64];

    type Level1BitBlock = u64;
    type Level1BlockIndex = u8;
    type Level1BlockIndices = [u16; 64];

    type DataBitBlock = u64;
    type DataBlockIndex = u16;
}

/// MAX = 2_097_152
#[cfg(feature = "simd")]
#[derive(Default)]
pub struct simd_128;

#[cfg(feature = "simd")]
impl IConfig for simd_128 {
    type Level0BitBlock = wide::u64x2;
    type Level0BlockIndices = [u8; 128];

    type Level1BitBlock = wide::u64x2;
    type Level1BlockIndex = u8;
    type Level1BlockIndices = [u16; 128];

    type DataBitBlock = wide::u64x2;
    type DataBlockIndex = u16;
}

// TODO: simd_256