//! Implementation details for customization.
//! 
//! # Custom bitblock
//! 
//! If your target architecture have some specific SIMD registers, which
//! you want to use as bitblocks, or you just want to have wider bitblocks to 
//! increase [BitSet] range - you can do this:
//!  * Implement [BitBlock] for your type.
//!  * Make [Config] with your bitblocks.
//!  * Use that config with [BitSet].
//! 
//! [BitSet]: crate::BitSet
//! [BitBlock]: crate::BitBlock
//! [Config]: crate::config::Config
//! 
//! # Custom bitset
//! 
//! You can make generative bitsets, like
//! "empty", "full", "range-fill", etc. with virtually zero memory overhead
//! and instant construction. 
//! 
//! Use [impl_bitset!] to make bitset from [LevelMasksIterExt].  
//! Use [impl_simple_bitset!] to make bitset from [LevelMasks].  
//! Otherwise, you can manually implement [BitSetInterface], and optionally other traits.
//!
//! [BitSetInterface]: crate::BitSetInterface
//! [impl_bitset!]: crate::impl_bitset
//! [impl_simple_bitset!]: crate::impl_bitset_simple
//! 
//! ```
//! # use std::marker::PhantomData;
//! # use std::mem::{ManuallyDrop, MaybeUninit};
//! # use hi_sparse_bitset::config::Config;
//! # use hi_sparse_bitset::{BitBlock, BitSetBase, BitSetInterface, impl_bitset};
//! # use hi_sparse_bitset::internals::*;
//! #[derive(Default)]
//! struct Empty<Conf: Config>(PhantomData<Conf>);
//!
//! impl<Conf: Config> BitSetBase for Empty<Conf> {
//!     type Conf = Conf;
//!     const TRUSTED_HIERARCHY: bool = true; 
//! }
//!
//! impl<Conf: Config> LevelMasks for Empty<Conf> {
//!     fn level0_mask(&self) -> <Self::Conf as Config>::Level0BitBlock {
//!         BitBlock::zero()
//!     }
//!
//!     unsafe fn level1_mask(&self, _: usize)
//!         -> <Self::Conf as Config>::Level1BitBlock 
//!     { 
//!         BitBlock::zero()
//!     }
//!
//!     unsafe fn data_mask(&self, _: usize, _: usize)
//!         -> <Self::Conf as Config>::DataBitBlock
//!     {
//!         BitBlock::zero()
//!     }
//! }
//!
//! // This is not needed with impl_bitset_simple!
//! impl<Conf: Config> LevelMasksIterExt for Empty<Conf> {
//!     type IterState = ();
//!     type Level1BlockData = ();
//!
//!     fn make_iter_state(&self) -> Self::IterState { () }
//!     unsafe fn drop_iter_state(&self, _: &mut ManuallyDrop<Self::IterState>) {}
//!
//!     unsafe fn init_level1_block_data(
//!         &self, 
//!         _: &mut Self::IterState, 
//!         _: &mut MaybeUninit<Self::Level1BlockData>, 
//!         _: usize
//!     ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
//!         (BitBlock::zero(), false)
//!     }
//!     
//!     unsafe fn data_mask_from_block_data(
//!         _: &Self::Level1BlockData, _: usize
//!     ) -> <Self::Conf as Config>::DataBitBlock {
//!         BitBlock::zero()
//!     }
//! }
//!
//! impl_bitset!(
//!     impl<Conf> for Empty<Conf> where Conf: Config
//! );
//! ```
//! 
//! See:
//! * examples/custom_bitset_simple.rs
//! * examples/custom_bitset.rs

use crate::bitset_interface::{bitset_is_empty, bitsets_eq, bitset_contains};

pub(crate) use crate::bitset_interface::LevelMasks;
pub(crate) use crate::bitset_interface::LevelMasksIterExt;

pub use crate::primitive::Primitive;

/// Can detect inequality earlier with [TRUSTED_HIERARCHY].
/// 
/// [TRUSTED_HIERARCHY]: crate::BitSetBase::TRUSTED_HIERARCHY
#[inline]
pub fn is_eq<L, R>(left: L, right: R) -> bool
where
    L: LevelMasksIterExt,
    R: LevelMasksIterExt<Conf = L::Conf>
{
    bitsets_eq(left, right)
}

/// O(1) for [TRUSTED_HIERARCHY].
/// 
/// [TRUSTED_HIERARCHY]: crate::BitSetBase::TRUSTED_HIERARCHY 
#[inline]
pub fn is_empty<S: LevelMasksIterExt>(bitset: S) -> bool {
    bitset_is_empty(bitset)
}

#[inline]
pub fn contains<S: LevelMasks>(bitset: S, index: usize) -> bool {
    bitset_contains(bitset, index)
}

/// Makes bitset from [LevelMasksIterExt].
/// 
/// Implements [BitSetInterface], [IntoIterator], [Eq], [Debug], [BitAnd], [BitOr], [BitXor], [Sub]
/// for [LevelMasksIterExt]. Also duplicates part of BitSetInterface in struct impl,
/// for ease of use. 
/// 
/// `ref` version will implement [BitSetInterface] for &T only. 
/// Otherwise - it will be implemented for both T and &T. 
/// Working only with refs will prevent T from being passed to apply/reduce
/// as value, and will allow to store `&self` pointer safely inside [Level1BlockData].
/// 
/// [BitAnd]: std::ops::BitAnd
/// [BitOr]: std::ops::BitOr
/// [BitXor]: std::ops::BitXor
/// [Sub]: std::ops::Sub
/// [BitSetInterface]: crate::BitSetInterface 
/// [BitSet]: crate::BitSet
/// [Level1BlockData]: LevelMasksIterExt::Level1BlockData
macro_rules! impl_bitset {
    (impl <$($generics:tt),*> for $t:ty) => {
        impl_bitset!(impl<$($generics),*> for $t where)
    };
    
    (impl <$($generics:tt),*> for $t:ty where $($where_bounds:tt)*) => {
        unsafe impl<$($generics),*> $crate::BitSetInterface for $t
        where
            $($where_bounds)*
        {}
        
        impl<$($generics),*> IntoIterator for $t
        where
            $($where_bounds)*
        {
            type Item = usize;
            type IntoIter = $crate::iter::IndexIter<Self>;

            #[inline]
            fn into_iter(self) -> Self::IntoIter {
                $crate::iter::IndexIter::new(self)
            }
        }        
        
        impl<$($generics),*, Rhs> std::ops::BitAnd<Rhs> for $t
        where
            Rhs: $crate::BitSetInterface<Conf = <Self as BitSetBase>::Conf>,
            $($where_bounds)*
        {
            type Output = $crate::Apply<$crate::ops::And, Self, Rhs>;

            /// Returns intersection of self and rhs bitsets.
            #[inline]
            fn bitand(self, rhs: Rhs) -> Self::Output{
                $crate::apply($crate::ops::And, self, rhs)
            }
        }
        
        impl<$($generics),*, Rhs> std::ops::BitOr<Rhs> for $t
        where
            Rhs: $crate::BitSetInterface<Conf = <Self as BitSetBase>::Conf>,
            $($where_bounds)*         
        {
            type Output = $crate::Apply<$crate::ops::Or, Self, Rhs>;

            /// Returns union of self and rhs bitsets.
            #[inline]
            fn bitor(self, rhs: Rhs) -> Self::Output{
                $crate::apply($crate::ops::Or, self, rhs)
            }
        }    
        
        impl<$($generics),*, Rhs> std::ops::BitXor<Rhs> for $t
        where
            Rhs: $crate::BitSetInterface<Conf = <Self as BitSetBase>::Conf>,
            $($where_bounds)*
        {
            type Output = $crate::Apply<$crate::ops::Xor, Self, Rhs>;

            /// Returns symmetric difference of self and rhs bitsets.
            #[inline]
            fn bitxor(self, rhs: Rhs) -> Self::Output{
                $crate::apply($crate::ops::Xor, self, rhs)
            }
        }
        
        impl<$($generics),*, Rhs> std::ops::Sub<Rhs> for $t
        where
            Rhs: $crate::BitSetInterface<Conf = <Self as BitSetBase>::Conf>,
            $($where_bounds)*         
        {
            type Output = $crate::Apply<$crate::ops::Sub, Self, Rhs>;

            /// Returns difference of self and rhs bitsets. 
            ///
            /// _Or relative complement of rhs in self._
            #[inline]
            fn sub(self, rhs: Rhs) -> Self::Output{
                $crate::apply($crate::ops::Sub, self, rhs)
            }
        }        
        
        impl_bitset!(impl<$($generics),*> for ref $t where $($where_bounds)*);
    };
    
    (impl <$($generics:tt),*> for ref $t:ty where $($where_bounds:tt)*) => {
        // --------------------------------
        // BitsetInterface
        unsafe impl<$($generics),*> $crate::BitSetInterface for &$t
        where
            $($where_bounds)*
        {}
        
        // --------------------------------
        // Duplicate BitsetInterface (not strictly necessary, but ergonomic)
        impl<$($generics),*> $t
        where
            $($where_bounds)*
        {
            #[inline]
            pub fn block_iter<'a>(&'a self) -> $crate::iter::BlockIter<&'a Self> 
            {
                $crate::iter::BlockIter::new(self)
            }   
            
            #[inline]
            pub fn iter<'a>(&'a self) -> $crate::iter::IndexIter<&'a Self> 
            {
                $crate::iter::IndexIter::new(self)
            }
            
            #[inline]
            pub fn contains(&self, index: usize) -> bool {
                $crate::internals::contains(self, index)
            }
            
            /// See [BitSetInterface::is_empty()]
            ///
            /// [BitSetInterface::is_empty()]: crate::BitSetInterface::is_empty()
            #[inline]
            pub fn is_empty(&self) -> bool {
                $crate::internals::is_empty(self)
            }
        }
        
        // --------------------------------
        // IntoIterator
        impl<$($generics),*> IntoIterator for &$t
        where
            $($where_bounds)*
        {
            type Item = usize;
            type IntoIter = $crate::iter::IndexIter<Self>;

            #[inline]
            fn into_iter(self) -> Self::IntoIter {
                $crate::iter::IndexIter::new(self)
            }
        }
        
        // --------------------------------
        // Eq
        impl<$($generics),*,Rhs> PartialEq<Rhs> for $t
        where
            Rhs: $crate::internals::LevelMasksIterExt<Conf = <Self as $crate::BitSetBase>::Conf>,
            $($where_bounds)*
        {
            /// Works faster with [TRUSTED_HIERARCHY].
            ///
            /// [TRUSTED_HIERARCHY]: crate::bitset_interface::BitSetBase::TRUSTED_HIERARCHY
            #[inline]
            fn eq(&self, other: &Rhs) -> bool {
                $crate::internals::is_eq(self, other)
            }
        }        
        
        impl<$($generics),*> Eq for $t
        where
            $($where_bounds)*
        {}
        
        
        // --------------------------------
        // Debug
        impl<$($generics),*> std::fmt::Debug for $t
        where
            $($where_bounds)*
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_list().entries(self.iter()).finish()
            }
        }
        
        
        // ---------------------------------
        // And
        impl<$($generics),*, Rhs> std::ops::BitAnd<Rhs> for &$t
        where
            Rhs: $crate::BitSetInterface<Conf = <Self as $crate::BitSetBase>::Conf>,
            $($where_bounds)*         
        {
            type Output = $crate::Apply<$crate::ops::And, Self, Rhs>;

            /// Returns intersection of self and rhs bitsets.
            #[inline]
            fn bitand(self, rhs: Rhs) -> Self::Output{
                $crate::apply($crate::ops::And, self, rhs)
            }
        }
        
        // ---------------------------------
        // Or
        impl<$($generics),*, Rhs> std::ops::BitOr<Rhs> for &$t
        where
            Rhs: $crate::BitSetInterface<Conf = <Self as $crate::BitSetBase>::Conf>,
            $($where_bounds)*         
        {
            type Output = $crate::Apply<$crate::ops::Or, Self, Rhs>;

            /// Returns union of self and rhs bitsets.
            #[inline]
            fn bitor(self, rhs: Rhs) -> Self::Output{
                $crate::apply($crate::ops::Or, self, rhs)
            }
        }         
        
        // ---------------------------------
        // Xor
        impl<$($generics),*, Rhs> std::ops::BitXor<Rhs> for &$t
        where
            Rhs: $crate::BitSetInterface<Conf = <Self as $crate::BitSetBase>::Conf>,
            $($where_bounds)*         
        {
            type Output = $crate::Apply<$crate::ops::Xor, Self, Rhs>;

            /// Returns symmetric difference of self and rhs bitsets.
            #[inline]
            fn bitxor(self, rhs: Rhs) -> Self::Output{
                $crate::apply($crate::ops::Xor, self, rhs)
            }
        }
        
        // ---------------------------------
        // Sub
        impl<$($generics),*, Rhs> std::ops::Sub<Rhs> for &$t
        where
            Rhs: $crate::BitSetInterface<Conf = <Self as $crate::BitSetBase>::Conf>,
            $($where_bounds)*         
        {
            type Output = $crate::Apply<$crate::ops::Sub, Self, Rhs>;

            /// Returns difference of self and rhs bitsets. 
            ///
            /// _Or relative complement of rhs in self._
            #[inline]
            fn sub(self, rhs: Rhs) -> Self::Output{
                $crate::apply($crate::ops::Sub, self, rhs)
            }
        }
    };
}
pub(crate) use impl_bitset;