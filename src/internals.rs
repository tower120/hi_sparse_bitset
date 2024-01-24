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
use crate::config::{DefaultBlockIterator, DefaultIndexIterator};
use crate::bitset_interface::BitSetInterface;

#[cfg_attr(docsrs, doc(cfg(feature = "impl")))]
#[cfg(feature = "impl")]
pub use crate::bitset_interface::LevelMasks;
#[cfg(not(feature = "impl"))]
pub(crate) use crate::bitset_interface::LevelMasks;

#[cfg_attr(docsrs, doc(cfg(feature = "impl")))]
#[cfg(feature = "impl")]
pub use crate::bitset_interface::LevelMasksIterExt;
#[cfg(not(feature = "impl"))]
pub(crate) use crate::bitset_interface::LevelMasksIterExt;

pub use crate::primitive::Primitive;

pub mod bit_queue{
    pub use crate::bit_queue::*;
}

#[inline]
pub fn into_index_iter<T>(set: T) -> DefaultIndexIterator<T>
where
    T: BitSetInterface
{
    DefaultIndexIterator::new(set)
}

#[inline]
pub fn index_iter<'a, T>(set: &'a T) -> DefaultIndexIterator<&'a T>
where
    &'a T: BitSetInterface
{
    DefaultIndexIterator::new(set)
} 

#[allow(dead_code)]
#[inline]
pub fn into_block_iter<T>(set: T) -> DefaultBlockIterator<T>
where
    T: BitSetInterface
{
    DefaultBlockIterator::new(set)
}

#[inline]
pub fn block_iter<'a, T>(set: &'a T) -> DefaultBlockIterator<&'a T>
where
    &'a T: BitSetInterface
{
    DefaultBlockIterator::new(set)
} 

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

#[inline]
pub fn contains<S: LevelMasks>(bitset: S, index: usize) -> bool {
    bitset_contains(bitset, index)
} 

/// Same as [impl_bitset], but for [LevelMasks].  
/// 
/// Implements [LevelMasksIterExt] by routing all operations to [LevelMasks].
/// 
/// # Safety
/// 
/// **DO NOT** implement [BitSetInterface] for `$t`, since `impl_bitset_simple`'s
/// [LevelMasksIterExt] implementation stores pointer to Self in [Level1BlockData].
/// If "drain iterator" will move during iteration - that will invalidate 
/// [Level1BlockData]. 
/// You have to use [impl_bitset!] if you need `$t` to be [BitSetInterface].
/// 
/// [BitSetInterface]: crate::BitSetInterface
/// [Level1BlockData]: crate::internals::LevelMasksIterExt::Level1BlockData
#[cfg_attr(docsrs, doc(cfg(feature = "impl")))]
#[cfg(feature = "impl")]
#[macro_export]
macro_rules! impl_bitset_simple {
    (impl <$($generics:tt),*> for ref $t:ty where $($where_bounds:tt)*) => {
        impl<$($generics),*> $crate::internals::LevelMasksIterExt for $t
        where
            $($where_bounds)*
        {
            type IterState = ();
            
            type Level1BlockData = (Option<std::ptr::NonNull<Self>>, usize);
        
            fn make_iter_state(&self) -> Self::IterState { () }
            unsafe fn drop_iter_state(&self, state: &mut ManuallyDrop<Self::IterState>) {}
        
            #[inline]
            unsafe fn init_level1_block_data(
                &self, 
                state: &mut Self::IterState, 
                level1_block_data: &mut MaybeUninit<Self::Level1BlockData>, 
                level0_index: usize
            ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
                level1_block_data.write((Some(self.into()), level0_index));
                (self.level1_mask(level0_index), true)
            }
            
            #[inline]
            unsafe fn data_mask_from_block_data(
                level1_block_data: &Self::Level1BlockData, level1_index: usize
            ) -> <Self::Conf as Config>::DataBitBlock {
                let this = unsafe{ level1_block_data.0.unwrap_unchecked() }.as_ref();
                let level0_index = level1_block_data.1;
                this.data_mask(level0_index, level1_index)
            }
        }
        
        $crate::impl_bitset!(impl<$($generics),*> for ref $t where $($where_bounds)*);
    };
}
//pub(crate) use impl_bitset_simple;

/// Makes bitset from [LevelMasksIterExt].
/// 
/// Implements [BitSetInterface], [IntoIterator], [Eq], [Debug], [BitAnd], [BitOr], [BitXor], [Sub]
/// for [LevelMasksIterExt]. Also duplicates part of BitSetInterface in struct impl,
/// for ease of use. 
/// 
/// `ref` version will implement [BitSetInterface] for &T only. 
/// Otherwise - it will be implemented for both T and &T. 
/// Working only with refs will prevent T from being passed to apply/reduce
/// as value, and will allow to store &self safely inside [Level1BlockData].
/// 
/// [BitAnd]: std::ops::BitAnd
/// [BitOr]: std::ops::BitOr
/// [BitXor]: std::ops::BitXor
/// [Sub]: std::ops::Sub
/// [BitSetInterface]: crate::BitSetInterface 
/// [BitSet]: crate::BitSet
/// [Level1BlockData]: LevelMasksIterExt::Level1BlockData
#[cfg_attr(docsrs, doc(cfg(feature = "impl")))]
#[cfg_attr(feature = "impl", macro_export)]
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
            type IntoIter = $crate::iter::CachingIndexIter<Self>;

            #[inline]
            fn into_iter(self) -> Self::IntoIter {
                $crate::internals::into_index_iter(self)
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
            pub fn block_iter<'a>(&'a self) -> $crate::iter::CachingBlockIter<&'a Self> 
            {
                $crate::internals::block_iter(self)
            }   
            
            #[inline]
            pub fn iter<'a>(&'a self) -> $crate::iter::CachingIndexIter<&'a Self> 
            {
                $crate::internals::index_iter(self)
            }
            
            #[inline]
            pub fn contains(&self, index: usize) -> bool {
                $crate::internals::contains(self, index)
            }
            
            /// See [BitSetInterface::is_empty()]
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
            type IntoIter = $crate::iter::CachingIndexIter<Self>;

            #[inline]
            fn into_iter(self) -> Self::IntoIter {
                $crate::internals::into_index_iter(self)
            }
        }
        
        // --------------------------------
        // Eq
        impl<$($generics),*,Rhs> PartialEq<Rhs> for $t
        where
            Rhs: $crate::internals::LevelMasksIterExt<Conf = <Self as BitSetBase>::Conf>,
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
    };
}
pub(crate) use impl_bitset;