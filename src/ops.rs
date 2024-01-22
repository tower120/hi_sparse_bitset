//! Operations for [apply] and [reduce].
//!
//! * [And] is the only operation that can discard blocks early
//! on hierarchy level during traverse. Complexity-wise this is the fastest operation.
//! * [Or] - does not need to discard any blocks, since it is a merge operation by definition.
//! * [Xor] - have [Or] performance.
//! * [Sub] - traverse all left operand bitset blocks.
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
    /// Will operation between two TrustedHierarchy bitsets produce 
    /// TrustedHierarchy as well?
    /// 
    /// Enables some optimizations. False - is always safe value.
    const TRUSTED_HIERARCHY: bool;
    
    /// Does [hierarchy_op] operands contain result?
    /// - left contains all bits from [hierarchy_op] result,
    /// - right contains all bits from [hierarchy_op] result,
    /// 
    /// This is true for [intersection], or narrower.
    /// 
    /// Enables some optimizations. False - is always safe value.
    /// 
    /// [hierarchy_op]: Self::hierarchy_op
    /// [intersection]: And
    const HIERARCHY_OPERANDS_CONTAIN_RESULT: bool;
    
    /// Operation applied to indirection/hierarchy level bitblock
    fn hierarchy_op<T: BitBlock>(left: T, right: T) -> T;

    /// Operation applied to data level bitblock
    fn data_op<T: BitBlock>(left: T, right: T) -> T;
}

/// Intersection
/// 
/// Will traverse only intersected blocks of left and right.
#[derive(Default, Copy, Clone)]
pub struct And;
impl BitSetOp for And {
    const TRUSTED_HIERARCHY: bool = true;
    const HIERARCHY_OPERANDS_CONTAIN_RESULT: bool = true;
    
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
pub struct Or;
impl BitSetOp for Or {
    const TRUSTED_HIERARCHY: bool = true;
    const HIERARCHY_OPERANDS_CONTAIN_RESULT: bool = false;
    
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
/// Have performance of [Or].
#[derive(Default, Copy, Clone)]
pub struct Xor;
impl BitSetOp for Xor {
    const TRUSTED_HIERARCHY: bool = false;
    const HIERARCHY_OPERANDS_CONTAIN_RESULT: bool = false;
    
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
pub struct Sub;
impl BitSetOp for Sub {
    const TRUSTED_HIERARCHY: bool = false;
    const HIERARCHY_OPERANDS_CONTAIN_RESULT: bool = false;
    
    #[inline]
    fn hierarchy_op<T: BitBlock>(left: T, _right: T) -> T {
        left
    }

    #[inline]
    fn data_op<T: BitBlock>(left: T, right: T) -> T {
        left & (left ^ right)
    }
}