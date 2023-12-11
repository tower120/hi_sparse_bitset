use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::{ControlFlow, Range};
use std::ops::ControlFlow::Break;
use crate::{BitSet, DataBlock, level_indices};
use crate::binary_op::BinaryOp;
use crate::bit_block::BitBlock;
use crate::cache::ReduceCache;
use crate::config::{DefaultBlockIterator, Config};
use crate::iter::{BlockIterator, BlockIterCursor, IndexIterator, IndexIterCursor};
use crate::bitset_op::BitSetOp;
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
/// Like with most Rust iterators, traversing[^traverse_def] is somewhat faster
/// then iteration.
///
/// [^traverse_def]: Under "traverse" we understand function application for 
/// each element of bitset.
pub trait BitSetInterface: BitSetBase + IntoIterator<Item = usize> + LevelMasksExt {
    // TODO: traverse from
    
    /// This is 25% faster then block iterator.
    /// 
    /// If `f` returns [Break], traversal will stop, and function will
    /// return [Break]. [Continue] is returned otherwise.
    /// 
    /// [Break]: ControlFlow::Break
    /// [Continue]: ControlFlow::Continue
    fn block_traverse<F>(&self, f: F) -> ControlFlow<()>
    where
        F: FnMut(DataBlock<<Self::Conf as Config>::DataBitBlock>) -> ControlFlow<()>;
    
    /// Up to x2 times faster then iterator in micro-benchmarks.
    /// 
    /// If `f` returns [Break], traversal will stop, and function will
    /// return [Break]. [Continue] is returned otherwise.
    /// 
    /// [Break]: ControlFlow::Break
    /// [Continue]: ControlFlow::Continue
    fn traverse<F>(&self, f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>;
    
    type BlockIter<'a>: BlockIterator where Self: 'a;
    fn block_iter(&self) -> Self::BlockIter<'_>;

    type Iter<'a>: IndexIterator<Item = usize> where Self: 'a;
    fn iter(&self) -> Self::Iter<'_>;

    type IntoBlockIter: BlockIterator;
    fn into_block_iter(self) -> Self::IntoBlockIter;

    fn contains(&self, index: usize) -> bool;
}

impl<T: LevelMasksExt> BitSetInterface for T
where
    T: IntoIterator<Item = usize>
{
    #[inline]
    fn block_traverse<F>(&self, f: F) -> ControlFlow<()> 
    where 
        F: FnMut(DataBlock<<Self::Conf as Config>::DataBitBlock>) -> ControlFlow<()> 
    {
        traverse(self, f)
    }

    #[inline]
    fn traverse<F>(&self, mut f: F) -> ControlFlow<()> 
    where 
        F: FnMut(usize) -> ControlFlow<()> 
    {
        self.block_traverse(|block|
             block.traverse(|i|f(i))
        )
    }

    type BlockIter<'a> = DefaultBlockIterator<&'a T> where Self: 'a;

    #[inline]
    fn block_iter(&self) -> Self::BlockIter<'_> {
        DefaultBlockIterator::new(self)
    }

    type Iter<'a> = <Self::BlockIter<'a> as BlockIterator>::IndexIter where Self: 'a;

    #[inline]
    fn iter(&self) -> Self::Iter<'_> {
        self.block_iter().as_indices()
    }

    type IntoBlockIter = DefaultBlockIterator<T>;

    #[inline]
    fn into_block_iter(self) -> Self::IntoBlockIter {
        DefaultBlockIterator::new(self)
    }

    #[inline]
    fn contains(&self, index: usize) -> bool {
        let (level0_index, level1_index, data_index) = level_indices::<T::Conf>(index);
        unsafe{
            let data_block = self.data_mask(level0_index, level1_index);
            data_block.get_bit(data_index)
        }
    }
}

macro_rules! impl_all {
    ($macro_name: ident) => {
        $macro_name!(impl<Conf> for BitSet<Conf> where Conf: Config);
        $macro_name!(
            impl<Op, S1, S2> for BitSetOp<Op, S1, S2>
            where
                Op: BinaryOp,
                S1: LevelMasksExt<Conf = S2::Conf>,
                S2: LevelMasksExt
        );
        $macro_name!(
            impl<Op, S, Storage> for Reduce<Op, S, Storage>
            where
                Op: BinaryOp,
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
            impl<'a, Op, S1, S2> for &'a BitSetOp<Op, S1, S2>
            where
                Op: BinaryOp,
                S1: LevelMasksExt<Conf = S2::Conf>,
                S2: LevelMasksExt
        );
        $macro_name!(
            impl<'a, Op, S, Storage> for &'a Reduce<Op, S, Storage>
            where
                Op: BinaryOp,
                S: Iterator + Clone,
                S::Item: LevelMasksExt,
                Storage: ReduceCache
        );
    }
}


// TODO: consider using &mut f in helpers
#[inline]
pub(crate) fn level1_mask_traverse_fn<S, F>(
    level0_index: usize,
    level1_index: usize,
    level1_blocks: &MaybeUninit<S::Level1Blocks>,
    mut f: F
) -> ControlFlow<()>
where
    S: LevelMasksExt, 
    F: FnMut(DataBlock<<S::Conf as Config>::DataBitBlock>) -> ControlFlow<()>
{
    let data_mask = unsafe {
        S::data_mask_from_blocks(level1_blocks.assume_init_ref(), level1_index)
    };
    
    let block_start_index =
        crate::data_block_start_index::<<S as BitSetBase>::Conf>(
            level0_index, level1_index
        );

    f(DataBlock{ start_index: block_start_index, bit_block: data_mask })
}

#[inline]
pub(crate) fn level0_mask_traverse_fn<S, F>(
    set: &S,
    level0_index: usize,
    cache_data: &mut S::CacheData,
    level1_blocks: &mut MaybeUninit<S::Level1Blocks>,
    mut f: F
) -> ControlFlow<()>
where
    S: LevelMasksExt, 
    F: FnMut(DataBlock<<S::Conf as Config>::DataBitBlock>) -> ControlFlow<()>
{
    let level1_mask = unsafe {
        iter_update_level1_blocks(&set, cache_data, level1_blocks, level0_index)
    };
    
    level1_mask.traverse_bits(|level1_index|{
        level1_mask_traverse_fn::<S, _>(level0_index, level1_index, level1_blocks, |b| f(b))
    })
}

#[inline]
fn traverse<S, F>(set: &S, mut f: F) -> ControlFlow<()>
where
    S: LevelMasksExt, 
    F: FnMut(DataBlock<<S::Conf as Config>::DataBitBlock>) -> ControlFlow<()>
{
    let level0_mask = set.level0_mask();
    
    let mut cache_data = set.make_cache();
    let mut level1_blocks = MaybeUninit::uninit();
    
    level0_mask.traverse_bits(
        |level0_index| level0_mask_traverse_fn(
            set, level0_index, &mut cache_data, &mut level1_blocks, |b| f(b)
        )
    )
}

#[inline]
pub fn traverse_from<S, F>(set: &S, cursor: BlockIterCursor, mut f: F) -> ControlFlow<()>
where
    S: LevelMasksExt, 
    F: FnMut(DataBlock<<S::Conf as Config>::DataBitBlock>) -> ControlFlow<()>
{
    let level0_mask = set.level0_mask();
    
    let mut cache_data = set.make_cache();
    let mut level1_blocks = MaybeUninit::uninit();
    
    // 1. Traverse first data block
    if level0_mask.get_bit(cursor.level0_index){
        let level1_mask = unsafe {
            iter_update_level1_blocks(&set, &mut cache_data, &mut level1_blocks, cursor.level0_index)
        };
        
        let ctrl = level1_mask.traverse_bits_from(
            cursor.level1_next_index, 
            |level1_index| level1_mask_traverse_fn::<S, _>(
                cursor.level0_index, level1_index, &level1_blocks, |b| f(b)
            )
        );
        if ctrl.is_break(){
            return Break(());
        }
    }
    
    // 2. Traverse all next as usual
    level0_mask.traverse_bits_from(
        cursor.level0_index+1,
        |level0_index| level0_mask_traverse_fn(
            set, level0_index, &mut cache_data, &mut level1_blocks, |b| f(b)
        )
    )
}

#[inline]
pub fn traverse_index_from<S, F>(set: &S, cursor: IndexIterCursor, mut f: F) -> ControlFlow<()>
where
    S: LevelMasksExt, 
    F: FnMut(usize) -> ControlFlow<()>
{
    let level0_mask = set.level0_mask();
    
    let mut cache_data = set.make_cache();
    let mut level1_blocks = MaybeUninit::uninit();
    
    let level0_index = cursor.block_cursor.level0_index;
    
    // 1. Traverse first level1 block
    if level0_mask.get_bit(level0_index) {
        let level1_mask = unsafe {
            iter_update_level1_blocks(&set, &mut cache_data, &mut level1_blocks, level0_index)
        };
        
        // 2. Traverse first data block FROM
        if level1_mask.get_bit(cursor.block_cursor.level1_next_index){
            let level1_index = cursor.block_cursor.level1_next_index;
            let data_mask = unsafe {
                S::data_mask_from_blocks(level1_blocks.assume_init_ref(), level1_index)
            };
            
            let block_start_index =
                crate::data_block_start_index::<<S as BitSetBase>::Conf>(
                    cursor.block_cursor.level0_index, level1_index
                );
            
            let ctrl = data_mask.traverse_bits_from(cursor.data_next_index, |index|{
                f(block_start_index + index)
            });
            if ctrl.is_break(){
                return Break(());
            }
        }
        
        // 3. Traverse rest data blocks as usual
        let ctrl = level1_mask.traverse_bits_from(
            cursor.block_cursor.level1_next_index + 1, 
            |level1_index|{
                let data_mask = unsafe {
                    S::data_mask_from_blocks(level1_blocks.assume_init_ref(), level1_index)
                };
                
                let block_start_index =
                    crate::data_block_start_index::<<S as BitSetBase>::Conf>(
                        level0_index, level1_index
                    );
                
                data_mask.traverse_bits(|index|{
                    f(block_start_index + index)
                })               
            }
        );
        if ctrl.is_break(){
            return Break(());
        }
    }
    
    // 4. Traverse all other as usual
    level0_mask.traverse_bits_from(
        level0_index+1, |level0_index| level0_mask_traverse_fn(
            set, level0_index, &mut cache_data, &mut level1_blocks, 
            |block| block.bit_block.traverse_bits(|index| f(block.start_index + index))
        )
    )
}

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
            type IntoIter = <<Self as BitSetInterface>::IntoBlockIter as BlockIterator>::IndexIter;

            #[inline]
            fn into_iter(self) -> Self::IntoIter {
                self.into_block_iter().as_indices()
            }
        }
    };
}
impl_all!(impl_into_iter);
impl_all_ref!(impl_into_iter);