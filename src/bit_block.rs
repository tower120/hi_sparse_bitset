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
    const SIZE_POT_EXPONENT: usize;
    
    #[inline]
    /*const*/ fn size() -> usize {
        1 << Self::SIZE_POT_EXPONENT
    }

    fn zero() -> Self;
    fn is_zero(&self) -> bool;

    // We could actually use more universal as_array and as_array_mut instead.
    // But this seems to be unnecessary now.
    fn first_u64(&self) -> &u64;
    fn first_u64_mut(&mut self) -> &mut u64;

    /// Returns previous bit
    /// 
    /// `bit_index` is guaranteed to be valid
    fn set_bit<const BIT: bool>(&mut self, bit_index: usize) -> bool;

    /// `bit_index` is guaranteed to be valid
    fn get_bit(&self, bit_index: usize) -> bool;

    // TODO: This can be removed, since there is BitQueue::traverse
    //       which do the same and perform the same in optimized build.
    /// Returns [Break] if traverse was interrupted (`f` returns [Break]).
    /// 
    /// [Break]: ControlFlow::Break
    fn traverse_bits<F>(&self, f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>;

    type BitsIter: BitQueue;
    fn bits_iter(self) -> Self::BitsIter;

    /*type AsArray: AsRef<[u64]>;
    fn as_array_u64(&self) -> &Self::AsArray;*/
    
    fn count_ones(&self) -> usize;
}

impl BitBlock for u64{
    const SIZE_POT_EXPONENT: usize = 6;

    #[inline]
    fn zero() -> Self{
        0
    }

    #[inline]
    fn is_zero(&self) -> bool {
        *self == 0
    }

    #[inline]
    fn first_u64(&self) -> &u64 {
        self
    }
    
    #[inline]
    fn first_u64_mut(&mut self) -> &mut u64 {
        self
    }

    #[inline]
    fn set_bit<const BIT: bool>(&mut self, bit_index: usize) -> bool{
        unsafe{
            bit_utils::set_bit_unchecked::<BIT, _>(self, bit_index)
        }
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
    fn bits_iter(self) -> Self::BitsIter {
        PrimitiveBitQueue::new(self)
    }

    /*type AsArray = [u64; 1];

    #[inline]
    fn as_array_u64(&self) -> &Self::AsArray {
        unsafe {
            mem::transmute::<&u64, &[u64; 1]>(self)
        }
    }*/

    #[inline]
    fn count_ones(&self) -> usize {
        u64::count_ones(*self) as usize
    }
}

#[cfg(feature = "simd")]
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

    #[inline]
    fn first_u64(&self) -> &u64 {
        unsafe{ self.as_array_ref().get_unchecked(0) }
    }
    
    #[inline]
    fn first_u64_mut(&mut self) -> &mut u64 {
        unsafe{ self.as_array_mut().get_unchecked_mut(0) }
    }

    #[inline]
    fn set_bit<const BIT: bool>(&mut self, bit_index: usize) -> bool {
        unsafe{
            bit_utils::set_array_bit_unchecked::<BIT, _>(self.as_array_mut(), bit_index)
        }
    }

    #[inline]
    fn get_bit(&self, bit_index: usize) -> bool {
        unsafe{
            bit_utils::get_array_bit_unchecked(self.as_array_ref(), bit_index)
        }
    }

    #[inline]
    fn traverse_bits<F>(&self, f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>
    {
        let array = self.as_array_ref();
        bit_utils::traverse_array_one_bits(array, f)
    }  

    type BitsIter = ArrayBitQueue<u64, 2>;
    #[inline]
    fn bits_iter(self) -> Self::BitsIter {
        Self::BitsIter::new(self.to_array())
    }

    /*type AsArray = [u64; 2];

    #[inline]
    fn as_array_u64(&self) -> &[u64; 2] {
        self.as_array_ref()
    }*/

    #[inline]
    fn count_ones(&self) -> usize {
        // TODO: there is faster solutions for this http://0x80.pl/articles/sse-popcount.html
        let primitives = self.as_array_ref();
        let len = primitives[0].count_ones() + primitives[1].count_ones();
        len as usize
    }
}

#[cfg(feature = "simd")]
impl BitBlock for wide::u64x4{
    const SIZE_POT_EXPONENT: usize = 8;

    #[inline]
    fn zero() -> Self {
        wide::u64x4::ZERO
    }

    #[inline]
    fn is_zero(&self) -> bool {
        *self == Self::zero()
    }
    
    #[inline]
    fn first_u64(&self) -> &u64 {
        unsafe{ self.as_array_ref().get_unchecked(0) }
    }
    
    #[inline]
    fn first_u64_mut(&mut self) -> &mut u64 {
        unsafe{ self.as_array_mut().get_unchecked_mut(0) }
    }

    #[inline]
    fn set_bit<const BIT: bool>(&mut self, bit_index: usize) -> bool {
        unsafe{
            bit_utils::set_array_bit_unchecked::<BIT, _>(self.as_array_mut(), bit_index)
        }
    }

    #[inline]
    fn get_bit(&self, bit_index: usize) -> bool {
        unsafe{
            bit_utils::get_array_bit_unchecked(self.as_array_ref(), bit_index)
        }
    }

    #[inline]
    fn traverse_bits<F>(&self, f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>
    {
        let array = self.as_array_ref();
        bit_utils::traverse_array_one_bits(array, f)
    }

    type BitsIter = ArrayBitQueue<u64, 4>;
    #[inline]
    fn bits_iter(self) -> Self::BitsIter {
        Self::BitsIter::new(self.to_array())
    }

    #[inline]
    fn count_ones(&self) -> usize {
        // TODO: there is faster solutions for this http://0x80.pl/articles/sse-popcount.html
        let primitives = self.as_array_ref();
        let len = primitives[0].count_ones() + primitives[1].count_ones()
                + primitives[2].count_ones() + primitives[3].count_ones();
        len as usize
    }
}
