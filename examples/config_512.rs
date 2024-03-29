use std::{mem, slice};
use std::mem::ManuallyDrop;
use std::ops::{BitAnd, BitOr, BitXor};
use itertools::assert_equal;
use hi_sparse_bitset::BitBlock;
use hi_sparse_bitset::cache::FixedCache;
use hi_sparse_bitset::config::{Config, SmallConfig};
use hi_sparse_bitset::internals::bit_queue::ArrayBitQueue;

#[derive(Eq, PartialEq, Copy, Clone)]
#[repr(C)]
pub struct Block512(wide::u64x4, wide::u64x4);

impl BitAnd for Block512 {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(
            self.0 & rhs.0,
            self.1 & rhs.1,
        )
    }
}

impl BitOr for Block512 {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(
            self.0 | rhs.0,
            self.1 | rhs.1,
        )
    }
}

impl BitXor for Block512 {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(
            self.0 ^ rhs.0,
            self.1 ^ rhs.1,
        )
    }
}

impl BitBlock for Block512{
    const SIZE_POT_EXPONENT: usize = 9;

    #[inline]
    fn zero() -> Self {
        Self(wide::u64x4::ZERO, wide::u64x4::ZERO)
    }
    
    type BitsIter = ArrayBitQueue<u64, 8>;

    #[inline]
    fn into_bits_iter(self) -> Self::BitsIter {
        let array = unsafe{
            mem::transmute_copy(&ManuallyDrop::new(self))
        };
        ArrayBitQueue::new(array)
    }

    #[inline]
    fn as_array(&self) -> &[u64] {
        unsafe{ 
            slice::from_raw_parts(self.0.as_array_ref().as_ptr(), 8)
        }
    }

    #[inline]
    fn as_array_mut(&mut self) -> &mut [u64] {
        unsafe{ 
            slice::from_raw_parts_mut(self.0.as_array_mut().as_mut_ptr(), 8)
        }
    }
}

struct config_512;
impl Config for config_512{
    type Level0BitBlock = Block512;
    type Level0BlockIndices = [u16; 512];
    
    type Level1BitBlock = Block512;
    type Level1BlockIndices = [u32; 512];
    
    type DataBitBlock = Block512;
    type DefaultCache = FixedCache<32>;
}
impl SmallConfig for config_512{
    type Level1SmallBlockIndices  = [u32; 14];
    type Level1MaskU64Populations = [u8; 8];
}

fn main(){
    type BitSet = hi_sparse_bitset::BitSet<config_512>;
    let mut bitset: BitSet = Default::default();
    for i in 0..50_000_000{
        bitset.insert(i);
    }
    assert_equal(&bitset, 0..50_000_000);
}