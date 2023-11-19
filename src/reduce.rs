use std::any::TypeId;
use std::marker::PhantomData;
use std::{mem, slice};
use std::alloc::{dealloc, Layout};
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ptr::{drop_in_place, NonNull, null_mut};
use crate::{IConfig, LevelMasks};
use crate::binary_op::{BinaryOp, BitAndOp};
use crate::cache::ReduceCache;
use crate::bitset_interface::LevelMasksExt;

/// Bitsets iterator reduction, as lazy bitset.
///
/// Constructed by [reduce()] and [reduce_w_cache()].
#[derive(Clone)]
#[repr(transparent)]
pub struct Reduce<Op, S, Cache> {
    pub(crate) sets: S,
    pub(crate) phantom: PhantomData<(Op, Cache)>
}

impl<Op, S, Cache> LevelMasks for Reduce<Op, S, Cache>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasks
{
    type Config = <S::Item as LevelMasks>::Config;

    #[inline]
    fn level0_mask(&self) -> <Self::Config as IConfig>::Level0BitBlock {
        unsafe{
            self.sets.clone()
            .map(|set| set.level0_mask())
            .reduce(Op::hierarchy_op)
            .unwrap_unchecked()
        }
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize)
        -> <Self::Config as IConfig>::Level1BitBlock
    {
        unsafe{
            self.sets.clone()
            .map(|set| {
                set.level1_mask(level0_index)
            })
            .reduce(Op::hierarchy_op)
            .unwrap_unchecked()
        }
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize)
        -> <Self::Config as IConfig>::DataBitBlock
    {
        unsafe{
            self.sets.clone()
            .map(|set| {
                set.data_mask(level0_index, level1_index)
            })
            .reduce(Op::data_op)
            .unwrap_unchecked()
        }
    }
}

/// We need this layer of indirection in form of intermediate trait,
/// because of RUST not having template/generics specialization.
/// Otherwise - we would have LevelMasksExt specializations for each
/// cache type.
pub trait ReduceCacheImpl
{
    type Config: IConfig;
    type Set: LevelMasksExt<Config = Self::Config>;
    type Sets: Iterator<Item = Self::Set> + Clone;

    const EMPTY_LVL1_TOLERANCE: bool;

    /// Cache only used by DynamicCache
    type CacheData;
    fn make_cache(sets: &Self::Sets) -> Self::CacheData;
    fn drop_cache(sets: &Self::Sets, cache: &mut ManuallyDrop<Self::CacheData>);

    type Level1Blocks3;
    unsafe fn update_level1_blocks3(
        sets: &Self::Sets,
        cache: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks3>,
        level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool);
    unsafe fn data_mask_from_blocks3(
        level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock;
}

pub struct NonCachedImpl<Op, T>(PhantomData<(Op, T)>);
impl<Op, S> ReduceCacheImpl for NonCachedImpl<Op, S>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasksExt,
{
    type Config = <S::Item as LevelMasks>::Config;
    type Set = S::Item;
    type Sets = S;
    type CacheData = ();
    type Level1Blocks3 = (S, usize);

    /// We always return true.
    const EMPTY_LVL1_TOLERANCE: bool = true;

    #[inline]
    fn make_cache(_: &Self::Sets) -> Self::CacheData{ () }

    #[inline]
    fn drop_cache(_: &Self::Sets, _: &mut ManuallyDrop<Self::CacheData>) {}

    #[inline]
    unsafe fn update_level1_blocks3(
        sets: &Self::Sets,
        _: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks3>,
        level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        level1_blocks.write((sets.clone(), level0_index));

        let reduce: &Reduce<Op, S, ()> = mem::transmute(sets);
        (reduce.level1_mask(level0_index), true)
    }

    // TODO: try pass level0_index - we always have it during iteration.
    //      This will allow not to store it in `update_level1_blocks3`
    #[inline]
    unsafe fn data_mask_from_blocks3(
        level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock {
        let (sets, level0_index) = level1_blocks;

        let reduce: &Reduce<Op, S, ()> = mem::transmute(sets);
        reduce.data_mask(*level0_index, level1_index)
    }
}

#[inline(always)]
unsafe fn update_level1_blocks3<Op, Config, Sets>(
    _: Op,
    sets: &Sets,
    cache_ptr: *mut <Sets::Item as LevelMasksExt>::CacheData,
    level1_blocks_ptr: *mut MaybeUninit<<Sets::Item as LevelMasksExt>::Level1Blocks>,
    level0_index: usize
) -> (<Config as IConfig>::Level1BitBlock, usize/*len*/, bool/*is_empty*/)
where
    Op: BinaryOp,
    Config: IConfig,
    Sets: Iterator + Clone,
    Sets::Item: LevelMasksExt<Config = Config>,
{
    // TODO: try actual const.
    // intersection case can be optimized, since we know
    // that with intersection, there can be no
    // empty masks/blocks queried.
    //
    // P.S. should be const, but act as const anyway.
    let is_intersection = TypeId::of::<Op>() == TypeId::of::<BitAndOp>();

    // Overwrite only non-empty blocks.
    let mut cache_index = 0;
    let mut index = 0;
    let mask =
        sets.clone()
        .map(|set|{
            let (level1_mask, valid) = set.update_level1_blocks(
                &mut *cache_ptr.add(cache_index),
                &mut *level1_blocks_ptr.add(index),
                level0_index
            );

            if is_intersection{
                // assume(valid)
                if !valid{ std::hint::unreachable_unchecked(); }
                index += 1;
                cache_index = index;
            } else {
                index += valid as usize;
                cache_index += 1;
            }

            level1_mask
        })
        .reduce(Op::hierarchy_op)
        .unwrap_unchecked();

    let is_empty =
        if is_intersection{
            // assume index != 0
            if index==0 { std::hint::unreachable_unchecked(); }
            true
        } else {
            index!=0
        };

    (mask, index, is_empty)
}


#[inline]
unsafe fn data_mask_from_blocks3<Op, Set, Config>(
    //_: Op,
    slice: &[Set::Level1Blocks],
    level1_index: usize
) -> <Config as IConfig>::DataBitBlock
where
    Op: BinaryOp,
    Config: IConfig,
    Set: LevelMasksExt<Config = Config>,
{
    unsafe{
        slice.iter()
            .map(|set_level1_blocks|
                <Set as LevelMasksExt>::data_mask_from_blocks(
                    set_level1_blocks, level1_index
                )
            )
            .reduce(Op::data_op)
            // level1_blocks can not be empty, since then -
            // level1 mask will be empty, and there will be nothing to iterate.
            .unwrap_unchecked()
    }
}

#[inline]
unsafe fn construct_child_cache<Sets>(
    sets: &Sets,
    cache_data_ptr: *mut MaybeUninit<<Sets::Item as LevelMasksExt>::CacheData>
)
where
    Sets: Iterator + Clone,
    Sets::Item: LevelMasksExt
{
    let mut index = 0;
    for  set in sets.clone(){
        let cache_data_element = &mut *cache_data_ptr.add(index);
        cache_data_element.write(set.make_cache());
        index += 1;
    }
}

#[inline]
unsafe fn destruct_child_cache<Sets>(
    sets: &Sets,
    cache_data_ptr: *mut ManuallyDrop<<Sets::Item as LevelMasksExt>::CacheData>
)
where
    Sets: Iterator + Clone,
    Sets::Item: LevelMasksExt
{
    let mut index = 0;
    for  set in sets.clone(){
        let cache_data_element = &mut *cache_data_ptr.add(index);
        set.drop_cache(cache_data_element);
        index += 1;
    }
}

pub struct FixedCacheImpl<Op, S, const N: usize>(PhantomData<(Op, S)>)
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasksExt;

impl<Op, S, const N: usize> ReduceCacheImpl for FixedCacheImpl<Op, S, N>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasksExt,
{
    type Config = <S::Item as LevelMasks>::Config;
    type Set = S::Item;
    type Sets = S;

    /// We use Level1Blocks3 directly, but childs may have data.
    /// Will be ZST, if no-one use. size = sets.len().
    type CacheData = [MaybeUninit<<Self::Set as LevelMasksExt>::CacheData>; N];

    /// Never drop, since array contain primitives, or array of primitives.
    type Level1Blocks3 = (
        [MaybeUninit<<Self::Set as LevelMasksExt>::Level1Blocks>; N],
        usize
    );

    const EMPTY_LVL1_TOLERANCE: bool = false;

    #[inline]
    fn make_cache(sets: &Self::Sets) -> Self::CacheData {
        unsafe{
            let mut cache_data = MaybeUninit::<Self::CacheData>::uninit().assume_init();
            construct_child_cache(sets, cache_data.as_mut_ptr());
            mem::transmute(cache_data)
        }
    }

    #[inline]
    fn drop_cache(sets: &Self::Sets, cache: &mut ManuallyDrop<Self::CacheData>) {
        unsafe{
            destruct_child_cache(sets, cache.as_mut_ptr() as *mut _);
            ManuallyDrop::drop(cache);
        }
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        sets: &Self::Sets,
        cache_data: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks3>,
        level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        let (level1_blocks_storage, level1_blocks_len) = level1_blocks.assume_init_mut();
        // assume_init_mut array
        let cache_ptr = cache_data.as_mut_ptr() as *mut <Self::Set as LevelMasksExt>::CacheData;

        let (mask, len, is_empty) =
            update_level1_blocks3(Op::default(), sets, cache_ptr, level1_blocks_storage.as_mut_ptr(), level0_index);
        *level1_blocks_len = len;
        (mask, is_empty)
    }

    #[inline]
    unsafe fn data_mask_from_blocks3(
        level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock {
        let slice = std::slice::from_raw_parts(
            level1_blocks.0.as_ptr() as *const <Self::Set as LevelMasksExt>::Level1Blocks,
            level1_blocks.1
        );
        data_mask_from_blocks3::<Op, Self::Set, Self::Config>(slice, level1_index)
    }
}

pub struct DynamicCacheImpl<Op, S>(PhantomData<(Op, S)>);
impl<Op, S> ReduceCacheImpl for DynamicCacheImpl<Op, S>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasksExt
{
    type Config =  <S::Item as LevelMasks>::Config;
    type Set = S::Item;
    type Sets = S;

    /// Have two separate storages, to keep local storage tight, and fast to iterate
    type CacheData = (
        // self storage (POD elements), never drop.
        // Do not use Box here, since Rust treat Box as &mut
        UniqueArrayPtr<MaybeUninit<<Self::Set as LevelMasksExt>::Level1Blocks>>,

        // child cache
        Box<[ManuallyDrop<<Self::Set as LevelMasksExt>::CacheData>]>,
    );

    /// raw slice
    type Level1Blocks3 = (
        // This points to CacheData heap
        *const <Self::Set as LevelMasksExt>::Level1Blocks,
        usize
    );

    const EMPTY_LVL1_TOLERANCE: bool = false;

    #[inline]
    fn make_cache(sets: &Self::Sets) -> Self::CacheData {
        let len = sets.clone().count();
        let mut child_cache = UniqueArrayPtr::new_uninit(len);
        unsafe{
            construct_child_cache(sets, child_cache.as_mut_ptr());
        }

        // recast MaybeUninit -> ManuallyDrop
        let child_cache = unsafe{
            let mut storage = ManuallyDrop::new(child_cache);
            let storage_ptr = storage.as_mut_ptr() as *mut _;
            Box::from_raw(
                slice::from_raw_parts_mut(storage_ptr, len)
            )
        };

        (UniqueArrayPtr::new_uninit(len), child_cache)
    }

    #[inline]
    fn drop_cache(sets: &Self::Sets, cache: &mut ManuallyDrop<Self::CacheData>) {
        unsafe{
            destruct_child_cache(sets, cache.1.as_mut_ptr());
            ManuallyDrop::drop(cache);
        }
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        sets: &Self::Sets,
        cache_data: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks3>,
        level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        let (storage, child_cache) = cache_data;

        // assume_init_mut array
        let cache_ptr = child_cache.as_mut_ptr() as *mut _;

        let (mask, len, is_empty) =
            update_level1_blocks3(Op::default(), sets, cache_ptr, storage.as_mut_ptr(), level0_index);

        level1_blocks.write((
            // assume_init_ref array
            storage.as_ptr() as *const _,
            len
        ));

        (mask, is_empty)
    }

    #[inline]
    unsafe fn data_mask_from_blocks3(
        level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock {
        let slice = std::slice::from_raw_parts(
            level1_blocks.0, level1_blocks.1
        );
        data_mask_from_blocks3::<Op, Self::Set, Self::Config>(slice, level1_index)
    }
}


impl<Op, S, Cache> LevelMasksExt for Reduce<Op, S, Cache>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasksExt,
    Cache: ReduceCache
{
    type CacheData = <Cache::Impl<Op, S> as ReduceCacheImpl>::CacheData;
    type Level1Blocks = <Cache::Impl<Op, S> as ReduceCacheImpl>::Level1Blocks3;
    const EMPTY_LVL1_TOLERANCE: bool = <Cache::Impl<Op, S> as ReduceCacheImpl>::EMPTY_LVL1_TOLERANCE;

    #[inline]
    fn make_cache(&self) -> Self::CacheData {
        <Cache::Impl<Op, S> as ReduceCacheImpl>::
            make_cache(&self.sets)
    }

    #[inline]
    fn drop_cache(&self, cache: &mut ManuallyDrop<Self::CacheData>) {
        <Cache::Impl<Op, S> as ReduceCacheImpl>::
            drop_cache(&self.sets, cache)
    }

    #[inline]
    unsafe fn update_level1_blocks(
        &self,
        cache_data: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks>,
        level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        <Cache::Impl<Op, S> as ReduceCacheImpl>::
            update_level1_blocks3(&self.sets, cache_data, level1_blocks, level0_index)
    }

    #[inline]
    unsafe fn data_mask_from_blocks(level1_blocks: &Self::Level1Blocks, level1_index: usize) -> <Self::Config as IConfig>::DataBitBlock {
        <Cache::Impl<Op, S> as ReduceCacheImpl>::
            data_mask_from_blocks3(level1_blocks, level1_index)
    }
}



#[inline]
fn dangling(layout: Layout) -> NonNull<u8>{
    #[cfg(miri)]
    {
        layout.dangling()
    }
    #[cfg(not(miri))]
    {
        unsafe { NonNull::new_unchecked(layout.align() as *mut u8) }
    }
}


/// Same as Box<[T]>, but aliasable.
/// See https://github.com/rust-lang/unsafe-code-guidelines/issues/326
pub struct UniqueArrayPtr<T>(NonNull<T>, usize);
impl<T> UniqueArrayPtr<T>{
    #[inline]
    pub fn new_uninit(len: usize) -> UniqueArrayPtr<MaybeUninit<T>>{
        // this is const
        let layout = Layout::array::<MaybeUninit<T>>(len).unwrap();
        unsafe{
            let mem =
                // Do not alloc ZST.
                if layout.size() == 0{
                    dangling(layout).as_ptr()
                } else {
                    let mem = std::alloc::alloc(layout);
                    assert!(mem != null_mut(), "Memory allocation fault.");
                    mem
                };

            UniqueArrayPtr(
                NonNull::new_unchecked(mem as *mut MaybeUninit<T>),
                len
            )
        }
    }

    #[inline]
    pub fn as_ptr(&self) -> *const T{
        self.0.as_ptr() as *const T
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T{
        self.0.as_ptr()
    }

    #[inline]
    pub fn as_slice(&self) -> &[T]{
        unsafe{ slice::from_raw_parts(self.0.as_ptr(), self.1) }
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T]{
        unsafe{ slice::from_raw_parts_mut(self.0.as_ptr(), self.1) }
    }

    /// noop
    #[inline]
    pub fn into_boxed_slice(mut self) -> Box<[T]>{
        unsafe{ Box::from_raw(self.as_mut_slice()) }
    }
}

impl<T> UniqueArrayPtr<MaybeUninit<T>>{
    #[inline]
    pub unsafe fn assume_init(array: UniqueArrayPtr<MaybeUninit<T>>) -> UniqueArrayPtr<T>{
        let UniqueArrayPtr(mem, len) = array;
        UniqueArrayPtr(mem.cast(), len)
    }
}

impl<T> Drop for UniqueArrayPtr<T>{
    #[inline]
    fn drop(&mut self) {
        // 1. call destructor
        if mem::needs_drop::<T>(){
            unsafe{ drop_in_place(self.as_mut_slice()); }
        }

        // 2. dealloc
        unsafe{
            // we constructed with this layout, it MUST be fine.
            let layout = Layout::array::<T>(self.1).unwrap_unchecked();
            // Do not dealloc ZST.
            if layout.size() != 0{
                dealloc(self.0.as_ptr() as *mut u8, layout);
            }
        }
    }
}