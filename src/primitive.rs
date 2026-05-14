use std::fmt::Debug;
use std::ops::*;

use crate::primitive_array::PrimitiveArray;

// num_traits was just **TOO** hard to use with primitives...
// Cast from/to concrete primitive was a final nail into num_trait's coffin.
pub trait Primitive:
    Default
    + Add<Output = Self>
    + AddAssign
    + BitAnd<Output = Self>
    + BitAndAssign
    + BitOr<Output = Self>
    + BitOrAssign
    + BitXor<Output = Self>
    + BitXorAssign
    + Shl<Output = Self>
    + Shl<usize, Output = Self>
    + ShlAssign
    + Shr<Output = Self>
    + Shr<usize, Output = Self>
    + ShrAssign
    + Not<Output = Self>
    + Copy
    + Ord
    + Debug
    + 'static
{
    const MIN: Self;
    const MAX: Self;

    const ZERO: Self;
    const ONE : Self;

    fn from_usize(i: usize) -> Self;
    fn from_u64(i: u64) -> Self;
    fn from_u32(i: u32) -> Self;

    fn as_usize(self) -> usize;
    fn as_u64(self) -> u64;
    fn as_u32(self) -> u32;

    fn trailing_zeros(self) -> u32;
    fn wrapping_neg(self) -> Self;
    fn wrapping_add(self, rhs: Self) -> Self;

    type BytesArray: PrimitiveArray<Item=u8>;
    fn to_ne_bytes(self) -> Self::BytesArray;
    fn to_le_bytes(self) -> Self::BytesArray;

    fn from_le_bytes(bytes: Self::BytesArray) -> Self;

    fn to_le(self) -> Self;

    fn swap_bytes(self) -> Self;

    fn is_zero(self) -> bool;
}

macro_rules! impl_primitive {
    ($x:ty) => {
        impl Primitive for $x{
            const MIN: $x = <$x>::MIN;
            const MAX: $x = <$x>::MAX;

            const ZERO: Self = 0;
            const ONE : Self = 1;

            #[inline]
            fn from_usize(i: usize) -> Self {
                i as Self
            }

            #[inline]
            fn from_u64(i: u64) -> Self {
                i as Self
            }

            #[inline]
            fn from_u32(i: u32) -> Self {
                i as Self
            }

            #[inline]
            fn as_usize(self) -> usize {
                self as usize
            }

            #[inline]
            fn as_u64(self) -> u64 {
                self as u64
            }

            #[inline]
            fn as_u32(self) -> u32 {
                self as u32
            }

            #[inline]
            fn trailing_zeros(self) -> u32 {
                self.trailing_zeros()
            }

            #[inline]
            fn wrapping_neg(self) -> Self {
                self.wrapping_neg()
            }

            #[inline]
            fn wrapping_add(self, rhs: Self) -> Self {
                self.wrapping_add(rhs)
            }

            type BytesArray = [u8; {(Self::BITS/8) as usize}];

            #[inline]
            fn to_ne_bytes(self) -> Self::BytesArray{
                self.to_ne_bytes()
            }

            #[inline]
            fn to_le_bytes(self) -> Self::BytesArray{
                self.to_le_bytes()
            }

            #[inline]
            fn from_le_bytes(bytes: Self::BytesArray) -> Self{
                Self::from_le_bytes(bytes)
            }

            #[inline]
            fn to_le(self) -> Self{
                self.to_le()
            }

            #[inline]
            fn swap_bytes(self) -> Self {
                self.swap_bytes()
            }

            #[inline]
            fn is_zero(self) -> bool {
                self == 0
            }
        }
    };
}

impl_primitive!(u8);
impl_primitive!(u16);
impl_primitive!(u32);
impl_primitive!(u64);
impl_primitive!(usize);