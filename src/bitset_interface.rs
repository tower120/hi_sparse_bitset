use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::ControlFlow;
use std::fmt;
use crate::{BitSet, level_indices};
use crate::ops::BitSetOp;
use crate::bit_block::BitBlock;
use crate::cache::ReduceCache;
use crate::config::{DefaultBlockIterator, Config, DefaultIndexIterator};
use crate::apply::Apply;
use crate::reduce::Reduce;

// We have this separate trait with Config, to avoid making LevelMasks public.
pub trait BitSetBase {
    type Conf: Config;
}

/// Basic interface for accessing block masks. Can work with [SimpleIter].
pub trait LevelMasks: BitSetBase{
    fn level0_mask(&self) -> <Self::Conf as Config>::Level0BitBlock;

    /// # Safety
    ///
    /// index is not checked
    unsafe fn level1_mask(&self, level0_index: usize)
        -> <Self::Conf as Config>::Level1BitBlock;

    /// # Safety
    ///
    /// indices are not checked
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize)
        -> <Self::Conf as Config>::DataBitBlock;
}

/// More sophisticated masks interface, optimized for iteration speed, through
/// caching level1(pre-data level) block pointer. This also, allow to discard
/// sets with empty level1 blocks in final stage of getting data blocks.
///
/// For use with [CachingIter].
pub trait LevelMasksExt: LevelMasks{
    /// Consists from child caches + Self state.
    /// Fot internal use (ala state).
    type CacheData;

    /// Cached Level1Blocks for faster accessing DataBlocks,
    /// without traversing whole hierarchy for getting each block during iteration.
    ///
    /// This may have less elements then sets size, because empty can be skipped.
    ///
    /// Must be POD. (Drop will not be called)
    type Level1Blocks;

    /// Could [data_mask_from_blocks3] be called if [update_level1_blocks3]
    /// returned false?
    ///
    /// Mainly used by op.
    const EMPTY_LVL1_TOLERANCE: bool;

    fn make_cache(&self) -> Self::CacheData;

    /// Having separate function for drop not strictly necessary, since
    /// CacheData can actually drop itself. But! This allows not to store cache
    /// size within CacheData. Which makes FixedCache CacheData ZST, if its childs
    /// are ZSTs, and which makes cache construction and destruction noop. Which is
    /// important for short iteration sessions.
    fn drop_cache(&self, cache: &mut ManuallyDrop<Self::CacheData>);

    /// Update `level1_blocks` and
    /// return (Level1Mask, is_not_empty/valid).
    ///
    /// if level0_index valid - update `level1_blocks`.
    unsafe fn update_level1_blocks(
        &self,
        cache: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool);

    /// # Safety
    ///
    /// - indices are not checked
    /// - if ![EMPTY_LVL1_TOLERANCE] should not be called, if
    ///   [update_level1_blocks] returned false.
    unsafe fn data_mask_from_blocks(
        /*&self,*/ level1_blocks: &Self::Level1Blocks, level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock;
}

impl<'a, T: LevelMasks> BitSetBase for &'a T {
    type Conf = T::Conf;
}
impl<'a, T: LevelMasks> LevelMasks for &'a T {
    #[inline]
    fn level0_mask(&self) -> <Self::Conf as Config>::Level0BitBlock {
        <T as LevelMasks>::level0_mask(self)
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize)
        -> <Self::Conf as Config>::Level1BitBlock
    {
        <T as LevelMasks>::level1_mask(self, level0_index)
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize)
        -> <Self::Conf as Config>::DataBitBlock
    {
        <T as LevelMasks>::data_mask(self, level0_index, level1_index)
    }
}
impl<'a, T: LevelMasksExt> LevelMasksExt for &'a T {
    type Level1Blocks = T::Level1Blocks;

    const EMPTY_LVL1_TOLERANCE: bool = T::EMPTY_LVL1_TOLERANCE;

    type CacheData = T::CacheData;

    #[inline]
    fn make_cache(&self) -> Self::CacheData {
        <T as LevelMasksExt>::make_cache(self)
    }

    #[inline]
    fn drop_cache(&self, cache: &mut ManuallyDrop<Self::CacheData>) {
        <T as LevelMasksExt>::drop_cache(self, cache)
    }

    #[inline]
    unsafe fn update_level1_blocks(
        &self,
        cache_data: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        <T as LevelMasksExt>::update_level1_blocks(
            self, cache_data, level1_blocks, level0_index
        )
    }

    #[inline]
    unsafe fn data_mask_from_blocks(
        level1_blocks: &Self::Level1Blocks, level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock {
        <T as LevelMasksExt>::data_mask_from_blocks(
            level1_blocks, level1_index
        )
    }
}

/// Helper function
/// 
/// # Safety
/// 
/// Only safe to call if you iterate `set`. 
/// (`set` at the top of lazy bitset operations hierarchy)
#[inline] 
pub(crate) unsafe fn iter_update_level1_blocks<S: LevelMasksExt>(
    set: &S,
    cache_data: &mut S::CacheData,
    level1_blocks: &mut MaybeUninit<S::Level1Blocks>,
    level0_index: usize    
) -> <S::Conf as Config>::Level1BitBlock{
    let (level1_mask, valid) = unsafe {
        set.update_level1_blocks(cache_data, level1_blocks, level0_index)
    };
    if !valid {
        // level1_mask can not be empty here
        unsafe { std::hint::unreachable_unchecked() }
    }
    level1_mask
}

// User-side interface
/// Bitset interface.
/// 
/// # Traversing
/// 
/// [CachingBlockIter] and [CachingIndexIter] have specialized `for_each()` implementation and `traverse()`.
/// 
/// Like with most Rust iterators, traversing[^traverse_def] is somewhat faster
/// then iteration. In this particular case, it has noticeable difference in micro-benchmarks.
/// Remember, that iteration is already super-fast, and any tiny change become important at that scale.
/// Hence, this will have effect in really tight loops (like incrementing counter).
///
/// [^traverse_def]: Under "traverse" we understand function application for 
/// each element of bitset.
/// 
/// [CachingBlockIter]: crate::iter::CachingBlockIter
/// [CachingIndexIter]: crate::iter::CachingIndexIter
pub trait BitSetInterface
    : BitSetBase 
    + IntoIterator<IntoIter = DefaultIndexIterator<Self>> 
    + LevelMasksExt 
    + Sized
{
    fn block_iter(&self) -> DefaultBlockIterator<&'_ Self>;
    fn iter(&self) -> DefaultIndexIterator<&'_ Self>;
    fn into_block_iter(self) -> DefaultBlockIterator<Self>;
    fn contains(&self, index: usize) -> bool;
}

/// BitSet that have no pointers to empty blocks in hierarchy.
/// 
/// This is usually by default, but some virtual bitsets may break this rule,
/// for example all union operations are not `TrustedHierarchy`.
pub trait TrustedHierarchy: BitSetInterface {
    /// O(1)
    #[inline]
    fn is_empty(&self) -> bool {
        self.level0_mask().is_zero()
    }
}

macro_rules! impl_all {
    ($macro_name: ident) => {
        $macro_name!(impl<Conf> for BitSet<Conf> where Conf: Config);
        $macro_name!(
            impl<Op, S1, S2> for Apply<Op, S1, S2>
            where
                Op: BitSetOp,
                S1: LevelMasksExt<Conf = S2::Conf>,
                S2: LevelMasksExt
        );
        $macro_name!(
            impl<Op, S, Storage> for Reduce<Op, S, Storage>
            where
                Op: BitSetOp,
                S: Iterator + Clone,
                S::Item: LevelMasksExt,
                Storage: ReduceCache
        );        
    }
}

macro_rules! impl_all_ref {
    ($macro_name: ident) => {
        $macro_name!(impl<'a, Conf> for &'a BitSet<Conf> where Conf: Config);
        $macro_name!(
            impl<'a, Op, S1, S2> for &'a Apply<Op, S1, S2>
            where
                Op: BitSetOp,
                S1: LevelMasksExt<Conf = S2::Conf>,
                S2: LevelMasksExt
        );
        $macro_name!(
            impl<'a, Op, S, Storage> for &'a Reduce<Op, S, Storage>
            where
                Op: BitSetOp,
                S: Iterator + Clone,
                S::Item: LevelMasksExt,
                Storage: ReduceCache
        );
    }
}

macro_rules! impl_bitset_interface {
    (impl <$($generics:tt),*> for $t:ty where $($where_bounds:tt)*) => {
        impl<$($generics),*> BitSetInterface for $t
        where
            $($where_bounds)*
        {
            #[inline]
            fn block_iter(&self) -> crate::config::DefaultBlockIterator<&'_ Self> {
                crate::config::DefaultBlockIterator::new(self)
            }
        
            #[inline]
            fn iter(&self) -> crate::config::DefaultIndexIterator<&'_ Self> {
                crate::config::DefaultIndexIterator::new(self)
            }
        
            #[inline]
            fn into_block_iter(self) -> crate::config::DefaultBlockIterator<Self> {
                crate::config::DefaultBlockIterator::new(self)
            }
        
            #[inline]
            fn contains(&self, index: usize) -> bool {
                let (level0_index, level1_index, data_index) = 
                    level_indices::<<Self as BitSetBase>::Conf>(index);
                unsafe{
                    let data_block = self.data_mask(level0_index, level1_index);
                    data_block.get_bit(data_index)
                }
            }            
        }     
    }    
}
impl_all!(impl_bitset_interface);
impl_all_ref!(impl_bitset_interface);

/// Duplicate/forward part of BitSetInterface to prevent the need of it's import.  
macro_rules! duplicate_bitset_interface {
    (impl <$($generics:tt),*> for $t:ty where $($where_bounds:tt)*) => {
        impl<$($generics),*> $t
        where
            $($where_bounds)*
        {
            #[inline]
            pub fn block_iter<'a>(&'a self) -> crate::config::DefaultBlockIterator<&'a Self> 
            {
                crate::config::DefaultBlockIterator::new(self)
            }   
            
            #[inline]
            pub fn iter<'a>(&'a self) -> crate::config::DefaultIndexIterator<&'a Self> 
            {
                crate::config::DefaultIndexIterator::new(self)
            }
            
            #[inline]
            pub fn contains(&self, index: usize) -> bool {
                BitSetInterface::contains(self, index)
            }
        }
    }
}
impl_all!(duplicate_bitset_interface);

// Optimistic depth-first check.
fn bitsets_eq<L, R>(left: L, right: R) -> bool
where
    L: LevelMasksExt,
    R: LevelMasksExt<Conf = L::Conf>,
{
    let left_level0_mask  = left.level0_mask();
    let right_level0_mask = right.level0_mask();
    
    if left_level0_mask != right_level0_mask {
        return false;
    }
    
    let mut left_cache_data  = left.make_cache();
    let mut right_cache_data = right.make_cache();
    
    let mut left_level1_blocks  = MaybeUninit::uninit();
    let mut right_level1_blocks = MaybeUninit::uninit();
    
    use ControlFlow::*;
    left_level0_mask.traverse_bits(|level0_index|{
        let left_level1_mask = unsafe {
            iter_update_level1_blocks(&left, &mut left_cache_data, &mut left_level1_blocks, level0_index)
        };
        let right_level1_mask  = unsafe {
            iter_update_level1_blocks(&right, &mut right_cache_data, &mut right_level1_blocks, level0_index)
        };
        
        if left_level1_mask != right_level1_mask {
            return Break(()); 
        }
        
        left_level1_mask.traverse_bits(|level1_index|{
            let left_data = unsafe {
                L::data_mask_from_blocks(left_level1_blocks.assume_init_ref(), level1_index)
            };
            let right_data = unsafe {
                R::data_mask_from_blocks(right_level1_blocks.assume_init_ref(), level1_index)
            };
            
            if left_data == right_data{
                Continue(())
            }  else {
                Break(())                 
            }
        })
    }).is_continue()
}

macro_rules! impl_eq {
    (impl <$($generics:tt),*> for $t:ty where $($where_bounds:tt)*) => {
        impl<$($generics),*,Rhs> PartialEq<Rhs> for $t
        where
            $($where_bounds)*,
            Rhs: BitSetInterface<Conf = <Self as BitSetBase>::Conf>
        {
            #[inline]
            fn eq(&self, other: &Rhs) -> bool {
                bitsets_eq(self, other)
            }
        }        
        
        impl<$($generics),*> Eq for $t
        where
            $($where_bounds)*
        {} 
    }
}
impl_all!(impl_eq);

macro_rules! impl_into_iter {
    (impl <$($generics:tt),*> for $t:ty where $($where_bounds:tt)*) => {
        impl<$($generics),*> IntoIterator for $t
        where
            $($where_bounds)*
        {
            type Item = usize;
            type IntoIter = DefaultIndexIterator<Self>;

            #[inline]
            fn into_iter(self) -> Self::IntoIter {
                DefaultIndexIterator::new(self)
            }
        }
    };
}
impl_all!(impl_into_iter);
impl_all_ref!(impl_into_iter);

macro_rules! impl_debug {
    (impl <$($generics:tt),*> for $t:ty where $($where_bounds:tt)*) => {
        impl<$($generics),*> fmt::Debug for $t
        where
            $($where_bounds)*
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list().entries(self.iter()).finish()
            }
        }
    };
}
impl_all!(impl_debug);