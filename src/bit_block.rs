use std::mem;
use std::ops::{BitAnd, BitOr, BitXor, ControlFlow};
use crate::bit_utils;
use crate::bit_queue::{ArrayBitQueue, BitQueue, PrimitiveBitQueue};

// TODO: consider removing copy
/// Bit block.
///
/// Used in [Config], to define bit blocks [BitSet] is built from. 
/// 
/// [Config]: crate::config::Config
/// [BitSet]: crate::BitSet
pub trait BitBlock
    : BitAnd<Output = Self> + BitOr<Output = Self> + BitXor<Output = Self>
    + Eq + PartialEq
    + Sized + Copy + Clone
{
    /// 2^N bits
    const SIZE_POT_EXPONENT: usize;
    
    /// Size in bits
    #[inline]
    /*const*/ fn size() -> usize {
        1 << Self::SIZE_POT_EXPONENT
    }

    fn zero() -> Self;
    
    #[inline]
    fn is_zero(&self) -> bool {
        self == &Self::zero()
    }

    /// Returns (previous bit, edited u64).
    /// 
    /// "edited u64" used as a hint for block emptiness/fullness after `set_bit`.
    /// 
    /// `bit_index` is guaranteed to be valid
    #[inline]
    fn set_bit<const BIT: bool>(&mut self, bit_index: usize) -> (bool, u64) {
        unsafe{
            bit_utils::set_array_bit_unchecked::<BIT, _>(self.as_array_mut(), bit_index)
        }
    }

    /// `bit_index` is guaranteed to be valid
    #[inline]
    fn get_bit(&self, bit_index: usize) -> bool{
        unsafe{
            bit_utils::get_array_bit_unchecked(self.as_array(), bit_index)
        }
    }

    // TODO: This can be removed, since there is BitQueue::traverse
    //       which do the same and perform the same in optimized build.
    /// Returns [Break] if traverse was interrupted (`f` returns [Break]).
    /// 
    /// [Break]: ControlFlow::Break
    #[inline]
    fn traverse_bits<F>(&self, f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>
    {
        bit_utils::traverse_one_bits_array(self.as_array(), f)
    }

    type BitsIter: BitQueue;
    fn into_bits_iter(self) -> Self::BitsIter;
    
    fn as_array(&self) -> &[u64];
    fn as_array_mut(&mut self) -> &mut [u64];
    
    #[inline]
    fn count_ones(&self) -> usize {
        let mut sum = 0;
        // will be unrolled at compile time
        for &i in self.as_array(){
            sum += u64::count_ones(i);
        } 
        sum as usize
    }
}

pub trait BitBlockFull: BitBlock{
    /// All bits 1.
    fn full() -> Self;
    
    #[inline]
    fn is_full(&self) -> bool{
        self == &Self::full()
    }
}

impl BitBlock for u64{
    const SIZE_POT_EXPONENT: usize = 6;

    #[inline]
    fn zero() -> Self{
        0
    }

    #[inline]
    fn set_bit<const BIT: bool>(&mut self, bit_index: usize) -> (bool, u64) {
        unsafe{(
            bit_utils::set_bit_unchecked::<BIT, _>(self, bit_index),
            *self
        )}
    }

    #[inline]
    fn get_bit(&self, bit_index: usize) -> bool {
        unsafe{
            bit_utils::get_bit_unchecked(*self, bit_index)
        }
    }

    #[inline]
    fn traverse_bits<F>(&self, f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>
    {
        bit_utils::traverse_one_bits(*self, f)
    }

    type BitsIter = PrimitiveBitQueue<u64>;
    #[inline]
    fn into_bits_iter(self) -> Self::BitsIter {
        PrimitiveBitQueue::new(self)
    }

    #[inline]
    fn as_array(&self) -> &[u64] {
        unsafe {
            mem::transmute::<&u64, &[u64; 1]>(self)
        }        
    }

    #[inline]
    fn as_array_mut(&mut self) -> &mut [u64] {
        unsafe {
            mem::transmute::<&mut u64, &mut [u64; 1]>(self)
        }        
    }
}

impl BitBlockFull for u64{
    #[inline]
    fn full() -> Self {
        u64::MAX
    }
}

#[cfg(feature = "simd")]
#[cfg_attr(docsrs, doc(cfg(feature = "simd")))]
impl BitBlock for wide::u64x2{
    const SIZE_POT_EXPONENT: usize = 7;

    #[inline]
    fn zero() -> Self {
        wide::u64x2::ZERO
    }

    #[inline]
    fn is_zero(&self) -> bool {
        // this should be faster then loading from memory into simd register,
        // and testz(if supported).
        let array = self.as_array_ref();
        (array[0] | array[1]) == 0
    }

    type BitsIter = ArrayBitQueue<u64, 2>;
    #[inline]
    fn into_bits_iter(self) -> Self::BitsIter {
        Self::BitsIter::new(self.to_array())
    }

    #[inline]
    fn as_array(&self) -> &[u64] {
        self.as_array_ref()
    }

    #[inline]
    fn as_array_mut(&mut self) -> &mut [u64] {
        self.as_array_mut()
    }
}
#[cfg(feature = "simd")]
#[cfg_attr(docsrs, doc(cfg(feature = "simd")))]
impl BitBlockFull for wide::u64x2{
    #[inline]
    fn full() -> Self {
        wide::u64x2::MAX
    }
}

#[cfg(feature = "simd")]
#[cfg_attr(docsrs, doc(cfg(feature = "simd")))]
impl BitBlock for wide::u64x4{
    const SIZE_POT_EXPONENT: usize = 8;

    #[inline]
    fn zero() -> Self {
        wide::u64x4::ZERO
    }

    type BitsIter = ArrayBitQueue<u64, 4>;
    #[inline]
    fn into_bits_iter(self) -> Self::BitsIter {
        Self::BitsIter::new(self.to_array())
    }

    #[inline]
    fn as_array(&self) -> &[u64] {
        self.as_array_ref()
    }

    #[inline]
    fn as_array_mut(&mut self) -> &mut [u64] {
        self.as_array_mut()
    }
}
#[cfg(feature = "simd")]
#[cfg_attr(docsrs, doc(cfg(feature = "simd")))]
impl BitBlockFull for wide::u64x4{
    #[inline]
    fn full() -> Self {
        wide::u64x4::MAX
    }
}
