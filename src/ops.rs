//! Operations for [apply] and [reduce].
//!
//! * [BitAndOp] is the only operation that can discard blocks early
//! on hierarchy level during traverse. Complexity-wise this is the fastest operation.
//! * [BitOrOp] - does not need to discard any blocks, since it is a merge operation by definition.
//! * [BitXorOp] - have [BitOrOp] performance.
//! * [BitSubOp] - traverse all left operand bitset blocks.
//!
//! You can make your own operation by implementing [BitSetOp].
//!
//! [apply]: crate::apply()
//! [reduce]: crate::reduce()

use std::ops::{BitAnd, BitOr, BitXor};
use crate::bit_block::BitBlock;

// TODO: all operations should accept & instead?
//       To work with [u64;N] more flawlessly?
/// Binary operation interface for [BitSetInterface]s.
///
/// Implement this trait for creating your own operation.
/// Pay attention to hierarchical nature of `hi_sparse_bitset` - you
/// may need to apply "broader" operations to "hierarchical blocks", then
/// to "data blocks".
/// 
/// [BitSetInterface]: crate::BitSetInterface
pub trait BitSetOp: Default + Copy + 'static{
    /// Operation applied to indirection/hierarchy level bitblock
    fn hierarchy_op<T: BitBlock>(left: T, right: T) -> T;

    /// Operation applied to data level bitblock
    fn data_op<T: BitBlock>(left: T, right: T) -> T;
}

/// Intersection
/// 
/// Will traverse only intersected blocks of left and right.
#[derive(Default, Copy, Clone)]
pub struct BitAndOp;
impl BitSetOp for BitAndOp {
    #[inline]
    fn hierarchy_op<T: BitBlock>(left: T, right: T) -> T {
        BitAnd::bitand(left, right)
    }

    #[inline]
    fn data_op<T: BitBlock>(left: T, right: T) -> T {
        BitAnd::bitand(left, right)
    }
}

/// Union
/// 
/// Will traverse all blocks of left and right. (Since all of them participate in merge)
#[derive(Default, Copy, Clone)]
pub struct BitOrOp;
impl BitSetOp for BitOrOp {
    #[inline]
    fn hierarchy_op<T: BitBlock>(left: T, right: T) -> T {
        BitOr::bitor(left, right)
    }

    #[inline]
    fn data_op<T: BitBlock>(left: T, right: T) -> T {
        BitOr::bitor(left, right)
    }
}

/// Symmetric difference.
/// 
/// Have performance of [BitOrOp].
#[derive(Default, Copy, Clone)]
pub struct BitXorOp;
impl BitSetOp for BitXorOp {
    #[inline]
    fn hierarchy_op<T: BitBlock>(left: T, right: T) -> T {
        BitOr::bitor(left, right)
    }

    #[inline]
    fn data_op<T: BitBlock>(left: T, right: T) -> T {
        BitXor::bitxor(left, right)
    }
}

/// Difference (relative complement) left\right.
/// 
/// Have performance of traversing left operand.
#[derive(Default, Copy, Clone)]
pub struct BitSubOp;
impl BitSetOp for BitSubOp {
    #[inline]
    fn hierarchy_op<T: BitBlock>(left: T, _right: T) -> T {
        left
    }

    #[inline]
    fn data_op<T: BitBlock>(left: T, right: T) -> T {
        left & (left ^ right)
    }
}