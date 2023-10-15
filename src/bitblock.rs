use std::ops::BitAnd;
use crate::bit_op;

// TODO: BitMap/BitMask/BitBlock instead?
pub trait BitBlock: BitAnd + Sized{
    const SIZE_POT_EXPONENT: usize;

    fn zero() -> Self;
    // TODO: is_empty?
    fn is_zero(&self) -> bool;

    /// Returns previous bit
    fn set_bit<const BIT: bool>(&mut self, bit_index: usize) -> bool;

    fn get_bit(&self, bit_index: usize) -> bool;
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
}