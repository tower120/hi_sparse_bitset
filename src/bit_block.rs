use std::mem;
use std::ops::{BitAnd, BitOr, BitXor, ControlFlow};
use crate::bit_op;
use crate::bit_queue::{ArrayBitQueue, ArrayBitQueue2, ArrayBitQueue3, BitQueue, PrimitiveBitQueue};

// TODO: consider removing copy/clone
pub trait BitBlock
    : BitAnd<Output = Self> + BitOr<Output = Self> + BitXor<Output = Self>
    + Sized + Copy + Clone
{
    const SIZE_POT_EXPONENT: usize;

    fn zero() -> Self;
    fn is_zero(&self) -> bool;

    /// Returns previous bit
    fn set_bit<const BIT: bool>(&mut self, bit_index: usize) -> bool;

    fn get_bit(&self, bit_index: usize) -> bool;

    fn traverse_bits<F>(&self, f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>;

    type BitsIter: BitQueue<Mask = Self::AsArray>;
    fn bits_iter(self) -> Self::BitsIter;

    type AsArray: AsRef<[u64]>;
    fn as_array_u64(&self) -> &Self::AsArray;
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
    fn set_bit<const BIT: bool>(&mut self, bit_index: usize) -> bool{
        bit_op::set_bit::<BIT, _>(self, bit_index)
    }

    #[inline]
    fn get_bit(&self, bit_index: usize) -> bool {
        bit_op::get_bit(*self, bit_index)
    }

    #[inline]
    fn traverse_bits<F>(&self, f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>
    {
        bit_op::traverse_one_bits(*self, f)
    }

    type BitsIter = PrimitiveBitQueue<u64>;
    #[inline]
    fn bits_iter(self) -> Self::BitsIter {
        PrimitiveBitQueue::new(self)
    }

    type AsArray = [u64; 1];

    #[inline]
    fn as_array_u64(&self) -> &Self::AsArray {
        unsafe {
            mem::transmute::<&u64, &[u64; 1]>(self)
        }
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
    fn set_bit<const BIT: bool>(&mut self, bit_index: usize) -> bool {
        bit_op::set_array_bit::<BIT, _>(self.as_array_mut(), bit_index)
    }

    #[inline]
    fn get_bit(&self, bit_index: usize) -> bool {
        bit_op::get_array_bit(self.as_array_ref(), bit_index)
    }

    #[inline]
    fn traverse_bits<F>(&self, f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>
    {
        let array = self.as_array_ref();
        bit_op::traverse_array_one_bits(array, f)
    }

    //type BitsIter = ArrayBitQueue<u64, 2>;
    //type BitsIter = ArrayBitQueue2<u64, 2, 1>;
    type BitsIter = ArrayBitQueue3<u64, 2>;
    #[inline]
    fn bits_iter(self) -> Self::BitsIter {
        Self::BitsIter::new(self.to_array())
    }

    type AsArray = [u64; 2];

    #[inline]
    fn as_array_u64(&self) -> &[u64; 2] {
        self.as_array_ref()
    }
}