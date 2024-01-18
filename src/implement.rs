//! Means for custom bitset implementation.
//! 
//! This allows you to make generative bitsets, like
//! "empty", "full", "range-fill", etc. with virtually zero memory overhead
//! and instant construction. 
//! 
//! Use [impl_bitset!] to make bitset from [LevelMasksIterExt].  
//! Use [impl_simple_bitset!] to make bitset from [LevelMasks].  
//! Otherwise, you can manually implement [BitSetInterface], and optionally other traits.
//!
//! [BitSetInterface]: crate::BitSetInterface   
//! 
//! # Example
//! 
//! ```
//! # use std::marker::PhantomData;
//! # use std::mem::{ManuallyDrop, MaybeUninit};
//! # use hi_sparse_bitset::config::Config;
//! # use hi_sparse_bitset::{BitBlock, BitSetBase, BitSetInterface, impl_bitset};
//! # use hi_sparse_bitset::implement::*;
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
//! // This is not needed with impl_simple_bitset!
//! impl<Conf: Config> LevelMasksIterExt for Empty<Conf> {
//!     type IterState = ();
//!     type Level1BlockData = ();
//! 
//!     fn make_iter_state(&self) -> Self::IterState { () }
//!     unsafe fn drop_iter_state(&self, _: &mut ManuallyDrop<Self::IterState>) {}
//! 
//!     unsafe fn update_level1_block_data(
//!         &self, 
//!         _: &mut Self::IterState, 
//!         _: &mut MaybeUninit<Self::Level1BlockData>, 
//!         _: usize
//!     ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
//!         (BitBlock::zero(), true)
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

use crate::bitset_interface::{bitset_is_empty, bitsets_eq};
pub use crate::bitset_interface::LevelMasks;
pub use crate::bitset_interface::LevelMasksIterExt;

#[inline]
pub fn is_eq<L, R>(left: L, right: R) -> bool
where
    L: LevelMasksIterExt,
    R: LevelMasksIterExt<Conf = L::Conf>
{
    bitsets_eq(left, right)
}

#[inline]
pub fn is_empty<S: LevelMasksIterExt>(bitset: S) -> bool {
    bitset_is_empty(bitset)
}

/// Same as [impl_bitset], but for [LevelMasks].  
/// 
/// Implements [LevelMasksIterExt] by routing all operations to [LevelMasks].
#[cfg_attr(feature = "impl", macro_export)]
macro_rules! impl_simple_bitset {
    (impl <$($generics:tt),*> for $t:ty where $($where_bounds:tt)*) => {
        impl<$($generics),*> $crate::implement::LevelMasksIterExt for $t
        where
            $($where_bounds)*
        {
            type IterState = ();
            // We can guarantee that self pointer remains valid,
            // since iterator holds reference to self.
            type Level1BlockData = (*const Self, usize);
        
            fn make_iter_state(&self) -> Self::IterState { () }
            unsafe fn drop_iter_state(&self, state: &mut ManuallyDrop<Self::IterState>) {}
        
            #[inline]
            unsafe fn update_level1_block_data(
                &self, 
                state: &mut Self::IterState, 
                level1_block_data: &mut MaybeUninit<Self::Level1BlockData>, 
                level0_index: usize
            ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
                level1_block_data.write((self, level0_index));
                (self.level1_mask(level0_index), true)
            }
            
            #[inline]
            unsafe fn data_mask_from_block_data(
                level1_block_data: &Self::Level1BlockData, level1_index: usize
            ) -> <Self::Conf as Config>::DataBitBlock {
                let this = unsafe{ &*level1_block_data.0 };
                let level0_index = level1_block_data.1;
                this.data_mask(level0_index, level1_index)
            }
        }
        
        impl_bitset!(impl<$($generics),*> for $t where $($where_bounds)*);
    };
}
pub(crate) use impl_simple_bitset;


/// Implements [BitSetInterface], [IntoIterator], [Eq], [Debug], [BitAnd], [BitOr], [BitXor], [Sub]
/// for [LevelMasksIterExt]. Also duplicates part of BitSetInterface in struct impl,
/// for ease of use. 
/// 
/// It will look like [BitSet], but without mutation operations.
/// 
/// [BitAnd]: std::ops::BitAnd
/// [BitOr]: std::ops::BitOr
/// [BitXor]: std::ops::BitXor
/// [Sub]: std::ops::Sub
/// [BitSetInterface]: crate::BitSetInterface 
/// [BitSet]: crate::BitSet
#[cfg_attr(feature = "impl", macro_export)]
macro_rules! impl_bitset {
    (impl <$($generics:tt),*> for $t:ty) => {
        impl_bitset!(impl<$($generics),*> for $t where)
    };
    
    (impl <$($generics:tt),*> for $t:ty where $($where_bounds:tt)*) => {
        // --------------------------------
        // BitsetInterface
        impl<$($generics),*> $crate::BitSetInterface for $t
        where
            $($where_bounds)*
        {}
        
        impl<$($generics),*> $crate::BitSetInterface for &$t
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
            pub fn block_iter<'a>(&'a self) -> $crate::iter::CachingBlockIter<&'a Self> 
            {
                $crate::iter::CachingBlockIter::new(self)
            }   
            
            #[inline]
            pub fn iter<'a>(&'a self) -> $crate::iter::CachingIndexIter<&'a Self> 
            {
                $crate::iter::CachingIndexIter::new(self)
            }
            
            #[inline]
            pub fn contains(&self, index: usize) -> bool {
                $crate::BitSetInterface::contains(self, index)
            }
            
            /// See [BitSetInterface::is_empty()]
            #[inline]
            pub fn is_empty(&self) -> bool {
                $crate::BitSetInterface::is_empty(self)
            }
        }
        
        
        // --------------------------------
        // IntoIterator
        impl<$($generics),*> IntoIterator for $t
        where
            $($where_bounds)*
        {
            type Item = usize;
            type IntoIter = $crate::iter::CachingIndexIter<Self>;

            #[inline]
            fn into_iter(self) -> Self::IntoIter {
                $crate::iter::CachingIndexIter::new(self)
            }
        }
        
        impl<$($generics),*> IntoIterator for &$t
        where
            $($where_bounds)*
        {
            type Item = usize;
            type IntoIter = $crate::iter::CachingIndexIter<Self>;

            #[inline]
            fn into_iter(self) -> Self::IntoIter {
                $crate::iter::CachingIndexIter::new(self)
            }
        }
        
        
        // --------------------------------
        // Eq
        impl<$($generics),*,Rhs> PartialEq<Rhs> for $t
        where
            Rhs: $crate::BitSetInterface<Conf = <Self as BitSetBase>::Conf>,
            $($where_bounds)*
        {
            /// Works faster with [TRUSTED_HIERARCHY].
            /// 
            /// [TRUSTED_HIERARCHY]: crate::bitset_interface::BitSetBase::TRUSTED_HIERARCHY
            #[inline]
            fn eq(&self, other: &Rhs) -> bool {
                $crate::implement::is_eq(self, other)
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
        
        impl<$($generics),*, Rhs> std::ops::BitAnd<Rhs> for &$t
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
        
        // ---------------------------------
        // Or
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
        
        impl<$($generics),*, Rhs> std::ops::BitOr<Rhs> for &$t
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
        
        // ---------------------------------
        // Xor
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
        
        impl<$($generics),*, Rhs> std::ops::BitXor<Rhs> for &$t
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
        
        // ---------------------------------
        // Sub
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
        
        impl<$($generics),*, Rhs> std::ops::Sub<Rhs> for &$t
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
        
        // TODO: 
    };
}
pub(crate) use impl_bitset;