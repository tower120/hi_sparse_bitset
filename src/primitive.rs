use std::fmt::Debug;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Shl, ShlAssign/*, Shr, ShrAssign*/};

// num_traits was just **TOO** hard to use with primitives...
// Cast from/to concrete primitive was a final nail into num_trait's coffin.
pub trait Primitive: 
    Default 
    + BitAnd<Output = Self>
    + BitAndAssign
    + BitOr<Output = Self>
    + BitOrAssign
    + BitXor<Output = Self>
    + BitXorAssign
    + Shl<Output = Self>
    + Shl<usize, Output = Self>
    + ShlAssign
/*    + Shr<Output = Self>
    + Shr<usize, Output = Self>
    + ShrAssign */
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
    fn as_usize(self) -> usize;
    
    fn trailing_zeros(self) -> u32;
    fn wrapping_neg(self) -> Self;
    
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
            fn as_usize(self) -> usize {
                self as usize
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