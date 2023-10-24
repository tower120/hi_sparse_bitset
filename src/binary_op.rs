use std::ops::{BitAnd};
use crate::bit_block::BitBlock;

pub trait BinaryOp{
    fn op<T: BitBlock>(left: T, right: T) -> T;
}

pub struct BitAndOp;
impl BinaryOp for BitAndOp {
    #[inline]
    fn op<T: BitBlock>(left: T, right: T) -> T {
        BitAnd::bitand(left, right)
    }
}
