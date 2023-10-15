use std::ops::{BitAndAssign, BitOrAssign};
use num_traits::int::PrimInt;

/// In machine endian.
#[inline]
pub fn set_bit<const FLAG: bool, T: PrimInt + BitAndAssign + BitOrAssign>(block: &mut T, bit_index: usize) -> bool {
    let block_mask: T = T::one() << bit_index;
    let masked_block = *block & block_mask;

    if FLAG {
        *block |= block_mask;
    } else {
        *block &= !block_mask;
    }

    !masked_block.is_zero()
}

/// In machine endian.
#[inline]
pub fn get_bit<T: PrimInt>(block: T, bit_index: usize) -> bool {
    let block_mask: T = T::one() << bit_index;
    let masked_block = block & block_mask;
    !masked_block.is_zero()
}
