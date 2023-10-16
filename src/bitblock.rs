use std::ops::{BitAnd, ControlFlow};
use std::ops::ControlFlow::*;
use crate::bit_op;

// TODO: consider removing copy/clone
pub trait BitBlock: BitAnd<Output = Self> + Sized + Copy + Clone{
    const SIZE_POT_EXPONENT: usize;

    fn zero() -> Self;
    // TODO: is_empty?
    fn is_zero(&self) -> bool;

    /// Returns previous bit
    fn set_bit<const BIT: bool>(&mut self, bit_index: usize) -> bool;

    fn get_bit(&self, bit_index: usize) -> bool;

    // TODO: consider removing
    fn traverse_bits<F>(&self, f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>;
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
    fn set_bit<const BIT: bool>(&mut self, mut bit_index: usize) -> bool {
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
}