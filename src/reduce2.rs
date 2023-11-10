use std::any::TypeId;
use std::marker::PhantomData;
use std::{mem, ptr};
use std::mem::MaybeUninit;
use crate::{IConfig, LevelMasks};
use crate::binary_op::{BinaryOp, BitAndOp};
use crate::cache::{CacheStorage, CacheStorageBuilder, FixedCache, FixedCacheStorage, NoCache};
use crate::iter::{CachingBlockIter, BlockIterator};
use crate::virtual_bitset::{LevelMasksExt3, LevelMasksRef};

#[derive(Clone)]
#[repr(transparent)]
pub struct Reduce<Op, S, Storage> {
    pub(crate) sets: S,
    pub(crate) phantom: PhantomData<(Op, Storage)>
}

impl<Op, S, Storage> LevelMasks for Reduce<Op, S, Storage>
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


pub trait ReduceCacheImpl
{
    type Config: IConfig;
    type Set: LevelMasksExt3<Config = Self::Config>;
    type Sets: Iterator<Item = Self::Set> + Clone;

    type CacheData;
    type Level1Blocks3;

    const EMPTY_LVL1_TOLERANCE: bool;

    fn make_cache(sets: &Self::Sets) -> Self::CacheData;
    fn drop_cache(sets: &Self::Sets, cache: Self::CacheData);

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

pub trait ReduceCacheImplBuilder{
    type Impl<Op, S>
        : ReduceCacheImpl<
            Sets = S,
            Config = <S::Item as LevelMasks>::Config
        >
    where
        Op: BinaryOp,
        S: Iterator + Clone,
        S::Item: LevelMasksExt3;
}

pub struct NonCachedImpl<Op, T>(PhantomData<(Op, T)>);
impl<Op, S> ReduceCacheImpl for NonCachedImpl<Op, S>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasksExt3,
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
    fn drop_cache(sets: &Self::Sets, _: Self::CacheData) {}

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

impl ReduceCacheImplBuilder for NoCache{
    type Impl<Op, S> = NonCachedImpl<Op, S>
    where
        Op: BinaryOp,
        S: Iterator + Clone,
        S::Item: LevelMasksExt3;
}

#[inline(always)]
unsafe fn update_level1_blocks3<Op, Config, Sets>(
    _: Op,
    sets: &Sets,
    cache_ptr: *mut <Sets::Item as LevelMasksExt3>::CacheData,
    level1_blocks_ptr: *mut MaybeUninit<<Sets::Item as LevelMasksExt3>::Level1Blocks3>,
    level0_index: usize
) -> (<Config as IConfig>::Level1BitBlock, usize/*len*/, bool/*is_empty*/)
where
    Op: BinaryOp,
    Config: IConfig,
    Sets: Iterator + Clone,
    Sets::Item: LevelMasksExt3<Config = Config>,
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
            let (level1_mask, valid) = set.update_level1_blocks3(
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
    slice: &[Set::Level1Blocks3],
    level1_index: usize
) -> <Config as IConfig>::DataBitBlock
where
    Op: BinaryOp,
    Config: IConfig,
    Set: LevelMasksExt3<Config = Config>,
{
    unsafe{
        slice.iter()
            .map(|set_level1_blocks|
                <Set as LevelMasksExt3>::data_mask_from_blocks3(
                    set_level1_blocks, level1_index
                )
            )
            .reduce(Op::data_op)
            // level1_blocks can not be empty, since then -
            // level1 mask will be empty, and there will be nothing to iterate.
            .unwrap_unchecked()
    }
}


pub struct FixedCacheImpl<Op, S, const N: usize>(PhantomData<(Op, S)>)
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasksExt3;

impl<Op, S, const N: usize> ReduceCacheImpl for FixedCacheImpl<Op, S, N>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasksExt3,
{
    type Config = <S::Item as LevelMasks>::Config;
    type Set = S::Item;
    type Sets = S;

    /// We use Level1Blocks3 directly, but childs may have data.
    /// Will be ZST, if no-one use. size = sets.len().
    type CacheData = [MaybeUninit<<Self::Set as LevelMasksExt3>::CacheData>; N];

    /// Never drop, since array contain primitives, or array of primitives.
    type Level1Blocks3 = (
        [MaybeUninit<<Self::Set as LevelMasksExt3>::Level1Blocks3>; N],
        usize
    );

    const EMPTY_LVL1_TOLERANCE: bool = false;

    #[inline]
    fn make_cache(sets: &Self::Sets) -> Self::CacheData {
        unsafe{
            let mut cache_data = MaybeUninit::<Self::CacheData>::uninit().assume_init();
            let mut cache_data_iter = cache_data.iter_mut();
            for  set in sets.clone(){
                let cache_data_element =
                    cache_data_iter.next().unwrap_unchecked();
                cache_data_element.write(set.make_cache());
            }
            mem::transmute(cache_data)
        }
    }

    #[inline]
    fn drop_cache(sets: &Self::Sets, cache: Self::CacheData) {
        if !mem::needs_drop::<Self::CacheData>(){
            return;
        }

        unsafe{
            let mut cache_data_iter = cache.into_iter();
            for  set in sets.clone(){
                let cache_data_element =
                    cache_data_iter.next().unwrap_unchecked();
                set.drop_cache(MaybeUninit::assume_init(cache_data_element));
            }
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
        let cache_ptr = cache_data.as_mut_ptr() as *mut <Self::Set as LevelMasksExt3>::CacheData;

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
            level1_blocks.0.as_ptr() as *const <Self::Set as LevelMasksExt3>::Level1Blocks3,
            level1_blocks.1
        );
        data_mask_from_blocks3::<Op, Self::Set, Self::Config>(slice, level1_index)
    }
}

impl<const N: usize> ReduceCacheImplBuilder for FixedCache<N>{
    type Impl<Op, S> = FixedCacheImpl<Op, S, N>
    where
        Op: BinaryOp,
        S: Iterator + Clone,
        S::Item: LevelMasksExt3;
}

// TODO: DynamicCache too !!


impl<Op, S, Storage> LevelMasksExt3 for Reduce<Op, S, Storage>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasksExt3,
    Storage: ReduceCacheImplBuilder
{
    type CacheData = <Storage::Impl<Op, S> as ReduceCacheImpl>::CacheData;
    type Level1Blocks3 = <Storage::Impl<Op, S> as ReduceCacheImpl>::Level1Blocks3;
    const EMPTY_LVL1_TOLERANCE: bool = <Storage::Impl<Op, S> as ReduceCacheImpl>::EMPTY_LVL1_TOLERANCE;

    #[inline]
    fn make_cache(&self) -> Self::CacheData {
        <Storage::Impl<Op, S> as ReduceCacheImpl>::
            make_cache(&self.sets)
    }

    #[inline]
    fn drop_cache(&self, cache: Self::CacheData) {
        <Storage::Impl<Op, S> as ReduceCacheImpl>::
            drop_cache(&self.sets, cache)
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        &self,
        cache_data: &mut Self::CacheData,
        level1_blocks: &mut MaybeUninit<Self::Level1Blocks3>,
        level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        <Storage::Impl<Op, S> as ReduceCacheImpl>::
            update_level1_blocks3(&self.sets, cache_data, level1_blocks, level0_index)
    }

    #[inline]
    unsafe fn data_mask_from_blocks3(level1_blocks: &Self::Level1Blocks3, level1_index: usize) -> <Self::Config as IConfig>::DataBitBlock {
        <Storage::Impl<Op, S> as ReduceCacheImpl>::
            data_mask_from_blocks3(level1_blocks, level1_index)
    }
}

impl<Op, S, Storage> LevelMasksRef for Reduce<Op, S, Storage>{}