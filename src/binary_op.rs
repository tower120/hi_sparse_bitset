//! [BitAndOp] is the only operation that can early discard
//! hierarchy/tree travers. Complexity-wise this is the fastest operation.
//!
//! All others does not discard hierarchical tree early.
//! _[BitOrOp] does not need to discard anything._

use std::ops::{BitAnd, BitOr, BitXor};
use crate::bit_block::BitBlock;

// TODO: all operations should accept & instead?
//       To work with [u64;N] more flawlessly?
pub trait BinaryOp: Copy + 'static{
    /// Operation applied to indirection/hierarchy level bitblock
    fn hierarchy_op<T: BitBlock>(left: T, right: T) -> T;

    /// Operation applied to data level bitblock
    fn data_op<T: BitBlock>(left: T, right: T) -> T;
}

#[derive(Copy, Clone)]
pub struct BitAndOp;
impl BinaryOp for BitAndOp {
    #[inline]
    fn hierarchy_op<T: BitBlock>(left: T, right: T) -> T {
        BitAnd::bitand(left, right)
    }

    #[inline]
    fn data_op<T: BitBlock>(left: T, right: T) -> T {
        BitAnd::bitand(left, right)
    }
}

#[derive(Copy, Clone)]
pub struct BitOrOp;
impl BinaryOp for BitOrOp {
    #[inline]
    fn hierarchy_op<T: BitBlock>(left: T, right: T) -> T {
        BitOr::bitor(left, right)
    }

    #[inline]
    fn data_op<T: BitBlock>(left: T, right: T) -> T {
        BitOr::bitor(left, right)
    }
}

/// Have performance of BitOrOp.
///
/// _Due to fact that hierarchy layers does not take part in culling symmetric difference._
#[derive(Copy, Clone)]
pub struct BitXorOp;
impl BinaryOp for BitXorOp {
    #[inline]
    fn hierarchy_op<T: BitBlock>(left: T, right: T) -> T {
        BitOr::bitor(left, right)
    }

    #[inline]
    fn data_op<T: BitBlock>(left: T, right: T) -> T {
        BitXor::bitxor(left, right)
    }
}

/// Have performance of traversing left operand.
#[derive(Copy, Clone)]
pub struct BitSubOp;
impl BinaryOp for BitSubOp {
    #[inline]
    fn hierarchy_op<T: BitBlock>(left: T, _right: T) -> T {
        left
    }

    #[inline]
    fn data_op<T: BitBlock>(left: T, right: T) -> T {
        left & (left ^ right)
    }
}