use std::ops::{BitAnd};
use crate::bit_block::BitBlock;

trait BinaryOp{
    fn op<T: BitBlock>(left: T, right: T) -> T;
}

struct BitAndOp;
impl BinaryOp for BitAndOp {
    #[inline]
    fn op<T: BitBlock>(left: T, right: T) -> T {
        BitAnd::bitand(left, right)
    }
}
