use std::ops::{BitAnd, BitOr, BitXor};
use crate::bit_block::BitBlock;

pub trait BinaryOp: Copy{
    fn op<T: BitBlock>(left: T, right: T) -> T;
}

#[derive(Copy, Clone)]
pub struct BitAndOp;
impl BinaryOp for BitAndOp {
    #[inline]
    fn op<T: BitBlock>(left: T, right: T) -> T {
        BitAnd::bitand(left, right)
    }
}

#[derive(Copy, Clone)]
pub struct BitOrOp;
impl BinaryOp for BitOrOp {
    #[inline]
    fn op<T: BitBlock>(left: T, right: T) -> T {
        BitOr::bitor(left, right)
    }
}

#[derive(Copy, Clone)]
pub struct BitXorOp;
impl BinaryOp for BitXorOp {
    #[inline]
    fn op<T: BitBlock>(left: T, right: T) -> T {
        BitXor::bitxor(left, right)
    }
}