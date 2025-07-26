use std::fmt::Debug;
use std::mem;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, ControlFlow};
use crate::bit_utils;
use crate::bit_queue::*;
use crate::primitive_array::PrimitiveArray;

#[cfg(feature = "serde")]
mod maybe_serde{
    pub use serde::Serialize as MaybeSerialize;
    pub use serde::Deserialize as MaybeDeserialize;
}

#[cfg(not(feature = "serde"))]
mod maybe_serde{
    pub trait MaybeSerialize {}
    impl<T> MaybeSerialize for T {}
    
    pub trait MaybeDeserialize<'de> {}
    impl<'de, T> MaybeDeserialize<'de> for T {}
}

use maybe_serde::*;

/// Bit block.
///
/// Used in [Config], to define bit blocks [BitSet] is built from. 
/// 
/// [Config]: crate::config::Config
/// [BitSet]: crate::BitSet
pub trait BitBlock
    : BitAnd<Output = Self>
    + BitAndAssign
    + BitOr<Output = Self>
    + BitOrAssign
    + BitXor<Output = Self>
    + BitXorAssign
    + Eq + PartialEq
    + MaybeSerialize + for<'de> MaybeDeserialize<'de>
    + Debug
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

    /// Returns previous bit
    /// 
    /// # Safety
    /// 
    /// `bit_index` must be < SIZE
    #[inline]
    unsafe fn set_bit_unchecked<const BIT: bool>(&mut self, bit_index: usize) -> bool {
        let array = self.as_array_mut().as_mut();
        bit_utils::set_array_bit_unchecked::<BIT, _>(array, bit_index)
    }

    /// # Safety
    /// 
    /// `bit_index` must be < SIZE
    #[inline]
    unsafe fn get_bit_unchecked(&self, bit_index: usize) -> bool{
        let array = self.as_array().as_ref();
        bit_utils::get_array_bit_unchecked(array, bit_index)    
    }

    // TODO: This can be removed, since there is BitQueue::traverse
    //       which do the same and perform the same in optimized build.
    /// Returns [Break] if traverse was interrupted (`f` returns [Break]).
    /// 
    /// [Break]: ControlFlow::Break
    #[inline]
    fn traverse_bits<F, B>(&self, f: F) -> ControlFlow<B>
    where
        F: FnMut(usize) -> ControlFlow<B>
    {
        let array = self.as_array().as_ref();
        bit_utils::traverse_array_one_bits(array, f)
    }
    
    #[inline]
    fn for_each_bit<F>(&self, mut f: F)
    where
        F: FnMut(usize)
    {
        let _ = self.traverse_bits(move |i| -> ControlFlow<()> {
            f(i);
            ControlFlow::Continue(())
        });
    }

    type BitsIter: BitQueue;
    fn into_bits_iter(self) -> Self::BitsIter;
    
    fn as_array(&self) -> &[u64];
    fn as_array_mut(&mut self) -> &mut [u64];
    
    type BytesArray: PrimitiveArray<Item=u8>;
    fn to_ne_bytes(self) -> Self::BytesArray;
    fn to_le_bytes(self) -> Self::BytesArray;
    fn from_ne_bytes(bytes: Self::BytesArray) -> Self;
    fn from_le_bytes(bytes: Self::BytesArray) -> Self;
    
    #[inline]
    fn to_le(self) -> Self {
        Self::from_le_bytes(self.to_le_bytes())
    }
    
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

impl BitBlock for u64{
    const SIZE_POT_EXPONENT: usize = 6;

    #[inline]
    fn zero() -> Self{
        0
    }

    #[inline]
    unsafe fn set_bit_unchecked<const BIT: bool>(&mut self, bit_index: usize) -> bool {
        bit_utils::set_bit_unchecked::<BIT, _>(self, bit_index)
    }

    #[inline]
    unsafe fn get_bit_unchecked(&self, bit_index: usize) -> bool {
        bit_utils::get_bit_unchecked(*self, bit_index)
    }

    #[inline]
    fn traverse_bits<F, B>(&self, f: F) -> ControlFlow<B>
    where
        F: FnMut(usize) -> ControlFlow<B>
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

    type BytesArray = [u8;8];
    #[inline]
    fn to_ne_bytes(self) -> Self::BytesArray {
        u64::to_ne_bytes(self)
    }    
    #[inline]
    fn to_le_bytes(self) -> Self::BytesArray {
        u64::to_le_bytes(self)
    }
    #[inline]
    fn from_ne_bytes(bytes: Self::BytesArray) -> Self {
        u64::from_ne_bytes(bytes)
    }
    #[inline]
    fn from_le_bytes(bytes: Self::BytesArray) -> Self {
        u64::from_le_bytes(bytes)
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
    
    type BytesArray = [u8;16];
    #[inline]
    fn to_ne_bytes(self) -> Self::BytesArray {
        // From rust doc:
        // "Because transmute is a by-value operation, alignment of the transmuted values themselves is not a concern".
        unsafe{ mem::transmute(self) }
    }
    #[inline]
    fn to_le_bytes(self) -> Self::BytesArray {
        #[cfg(target_endian = "little")]
        { self.to_ne_bytes() }
        #[cfg(target_endian = "big")]
        { unimplemented!() }
    }
    #[inline]
    fn from_ne_bytes(bytes: Self::BytesArray) -> Self {
        unsafe{ mem::transmute(bytes) }
    }
    #[inline]
    fn from_le_bytes(bytes: Self::BytesArray) -> Self {
        #[cfg(target_endian = "little")]
        { Self::from_ne_bytes(bytes) }
        #[cfg(target_endian = "big")]
        { unimplemented!() }
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
    
    type BytesArray = [u8;32];
    #[inline]
    fn to_ne_bytes(self) -> Self::BytesArray {
        // From rust doc:
        // "Because transmute is a by-value operation, alignment of the transmuted values themselves is not a concern".
        unsafe{ mem::transmute(self) }
    }
    #[inline]
    fn to_le_bytes(self) -> Self::BytesArray {
        #[cfg(target_endian = "little")]
        { self.to_ne_bytes() }
        #[cfg(target_endian = "big")]
        { unimplemented!() }
    }
    #[inline]
    fn from_ne_bytes(bytes: Self::BytesArray) -> Self {
        unsafe{ mem::transmute(bytes) }
    }
    #[inline]
    fn from_le_bytes(bytes: Self::BytesArray) -> Self {
        #[cfg(target_endian = "little")]
        { Self::from_ne_bytes(bytes) }
        #[cfg(target_endian = "big")]
        { unimplemented!() }
    }  
}
