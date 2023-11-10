use std::any::TypeId;
use std::marker::PhantomData;
use std::mem;
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

    type Level1Blocks3;

    const EMPTY_LVL1_TOLERANCE: bool;

    fn make_level1_blocks3(sets: &Self::Sets) -> Self::Level1Blocks3;

    unsafe fn update_level1_blocks3(
        sets: &Self::Sets, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
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
    type Level1Blocks3 = (S, usize);

    /// We always return true.
    const EMPTY_LVL1_TOLERANCE: bool = true;

    #[inline]
    fn make_level1_blocks3(sets: &Self::Sets) -> Self::Level1Blocks3 {
        (sets.clone(), 0)
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        sets: &Self::Sets, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        *level1_blocks = (sets.clone(), level0_index);

        let reduce: &Reduce<Op, S, ()> = mem::transmute(sets);
        (reduce.level1_mask(level0_index), true)
    }

    // TODO: pass level0_index - we always have it during iteration.
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
    level1_blocks_ptr: *mut <Sets::Item as LevelMasksExt3>::Level1Blocks3,
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

/*    // This should act the same as a few assumes in default loop,
    // but I feel safer this way.
    if TypeId::of::<Op>() == TypeId::of::<BitAndOp>() { /* compile-time check */
        let mut index = 0;
        let mask =
            sets.clone()
            .map(|set|{
                let (mask, valid) = set.update_level1_blocks3(
                    &mut *level1_blocks_ptr.add(index),
                    level0_index
                );
                // assume(valid)
                if !valid{ std::hint::unreachable_unchecked(); }
                index += 1;
                mask
            })
            .reduce(Op::hierarchy_op)
            .unwrap_unchecked();

        if index!=0{
            std::hint::unreachable_unchecked();
        }
        return (mask, index);
    }*/

    // Overwrite only non-empty blocks.
    let mut index = 0;
    let mask =
        sets.clone()
        .map(|set|{
            let (level1_mask, valid) = set.update_level1_blocks3(
                &mut *level1_blocks_ptr.add(index),
                level0_index
            );

            if is_intersection{
                // assume(valid)
                if !valid{ std::hint::unreachable_unchecked(); }
                index += 1;
            } else {
                index += valid as usize;
            }

            level1_mask
        })
        .reduce(Op::hierarchy_op)
        .unwrap_unchecked();

    let is_empty =
        if is_intersection{
            // assume
            if index!=0 { std::hint::unreachable_unchecked(); }
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
        /*let slice = std::slice::from_raw_parts(
            level1_blocks.0.as_ptr() as *const <Self::Set as LevelMasksExt3>::Level1Blocks3,
            level1_blocks.1
        );*/

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

pub struct FixedCacheImpl<Op, T, const N: usize>(PhantomData<(Op, T)>);
impl<Op, S, const N: usize> ReduceCacheImpl for FixedCacheImpl<Op, S, N>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasksExt3,
{
    type Config = <S::Item as LevelMasks>::Config;
    type Set = S::Item;
    type Sets = S;

    /// Never drop, since array contain primitives, or array of primitives.
    type Level1Blocks3 = (
        [MaybeUninit<<Self::Set as LevelMasksExt3>::Level1Blocks3>; N],
        usize
    );

    const EMPTY_LVL1_TOLERANCE: bool = false;

    #[inline]
    fn make_level1_blocks3(sets: &Self::Sets) -> Self::Level1Blocks3 {
        let mut storage: [MaybeUninit<<Self::Set as LevelMasksExt3>::Level1Blocks3>; N]
            = unsafe{ MaybeUninit::uninit().assume_init() };

        // init storage in deep
        unsafe{
            let mut index = 0;
            let elements = storage.as_mut_ptr();
            for set in sets.clone() {
                let element = elements.add(index);
                (*element).write(set.make_level1_blocks3());
                index += 1;
            }
            assert!(N >= index, "Reduce cache overflow");
        }

        return (storage, 0);
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        sets: &Self::Sets, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        let (level1_blocks_storage, level1_blocks_len) = level1_blocks;
        let level1_blocks_ptr = level1_blocks_storage.as_mut_ptr() as *mut <Self::Set as LevelMasksExt3>::Level1Blocks3;

        let (mask, len, is_empty) =
            update_level1_blocks3(Op::default(), sets, level1_blocks_ptr, level0_index);
        *level1_blocks_len = len;
        (mask, is_empty)

/*        // This should act the same as a few assumes in default loop,
        // but I feel safer this way.
        if TypeId::of::<Op>() == TypeId::of::<BitAndOp>() { /* compile-time check */
            // intersection case can be optimized, since we know
            // that with intersection, there can be no
            // empty masks/blocks queried.
            let mut index = 0;
            let mask =
                sets.clone()
                .map(|set|{
                    let (mask, valid) = set.update_level1_blocks3(
                        &mut *level1_blocks_ptr.add(index),
                        level0_index
                    );
                    // assume(valid)
                    if !valid{ std::hint::unreachable_unchecked(); }
                    index += 1;
                    mask
                })
                .reduce(Op::hierarchy_op)
                .unwrap_unchecked();

            *level1_blocks_len = index;
            return (mask, true);
        }

        // Overwrite only non-empty blocks.
        let mut index = 0;

        let mask_acc =
            sets.clone()
            .map(|set|{
                let (level1_mask, valid) = set.update_level1_blocks3(
                    &mut *level1_blocks_ptr.add(index),
                    level0_index
                );
                index += valid as usize;
                level1_mask
            })
            .reduce(Op::hierarchy_op)
            .unwrap_unchecked();

        *level1_blocks_len = index;
        (mask_acc, index !=0)*/
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

/*        unsafe{
            let slice = std::slice::from_raw_parts(
                level1_blocks.0.as_ptr() as *const <Self::Set as LevelMasksExt3>::Level1Blocks3,
                level1_blocks.1
            );

            slice.iter()
                .map(|set_level1_blocks|
                    <Self::Set as LevelMasksExt3>::data_mask_from_blocks3(
                        set_level1_blocks, level1_index
                    )
                )
                .reduce(Op::data_op)
                // level1_blocks can not be empty, since then -
                // level1 mask will be empty, and there will be nothing to iterate.
                .unwrap_unchecked()
        }*/
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
    type Level1Blocks3 = <Storage::Impl<Op, S> as ReduceCacheImpl>::Level1Blocks3;
    const EMPTY_LVL1_TOLERANCE: bool = <Storage::Impl<Op, S> as ReduceCacheImpl>::EMPTY_LVL1_TOLERANCE;

    #[inline]
    fn make_level1_blocks3(&self) -> Self::Level1Blocks3 {
        <Storage::Impl<Op, S> as ReduceCacheImpl>::
            make_level1_blocks3(&self.sets)
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        &self, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        <Storage::Impl<Op, S> as ReduceCacheImpl>::
            update_level1_blocks3(&self.sets, level1_blocks, level0_index)
    }

    #[inline]
    unsafe fn data_mask_from_blocks3(level1_blocks: &Self::Level1Blocks3, level1_index: usize) -> <Self::Config as IConfig>::DataBitBlock {
        <Storage::Impl<Op, S> as ReduceCacheImpl>::
            data_mask_from_blocks3(level1_blocks, level1_index)
    }
}



/*impl<Op, S, Storage> LevelMasksExt3 for Reduce<Op, S, Storage>
where
    Op: BinaryOp,
    S: Iterator + Clone,
    S::Item: LevelMasksExt3,
    Storage: CacheStorageBuilder
{
    type Level1Blocks3 = (

        // array of S::LevelMasksExt3
        //Storage::Level1Blocks3<S>,
        <Storage as CacheStorageBuilder>::Storage<<S::Item as LevelMasksExt3>::Level1Blocks3>,
        // len
        usize
    );

    const EMPTY_LVL1_TOLERANCE: bool = false;

    #[inline]
    fn make_level1_blocks3(&self) -> Self::Level1Blocks3 {
        // It should be faster to calculate sets amount in front,
        // then to relocated Vec with pushes during DynamicCache construction.
        let sets_count = || self.sets.clone().count();

        let mut storage: <Storage as CacheStorageBuilder>::Storage<<S::Item as LevelMasksExt3>::Level1Blocks3>
            = Storage::build(sets_count);

        // init storage in deep
        unsafe{
            let mut index = 0;
            let elements = storage.as_mut_ptr();
            for set in self.sets.clone() {
                let element = elements.add(index);
                std::ptr::write(
                    element,
                    set.make_level1_blocks3()
                );
                index += 1;
            }
            assert!(Storage::FIXED_CAPACITY >= index, "Reduce cache overflow");
        }

        return (storage, 0);
    }

    #[inline]
    unsafe fn update_level1_blocks3(
        &self, level1_blocks: &mut Self::Level1Blocks3, level0_index: usize
    ) -> (<Self::Config as IConfig>::Level1BitBlock, bool) {
        let (level1_blocks_storage, level1_blocks_len) = level1_blocks;
        let level1_blocks_ptr = level1_blocks_storage.as_mut_ptr();

        // This should act the same as a few assumes in default loop,
        // but I feel safer this way.
        if TypeId::of::<Op>() == TypeId::of::<BitAndOp>() { /* compile-time check */
            // intersection case can be optimized, since we know
            // that with intersection, there can be no
            // empty masks/blocks queried.
            let mut index = 0;
            let mask =
                self.sets.clone()
                .map(|set|{
                    let (mask, valid) = set.update_level1_blocks3(
                        &mut *level1_blocks_ptr.add(index),
                        level0_index
                    );
                    // assume(valid)
                    if !valid{ std::hint::unreachable_unchecked(); }
                    index += 1;
                    mask
                })
                .reduce(Op::hierarchy_op)
                .unwrap_unchecked();

            *level1_blocks_len = index;
            return (mask, true);
        }

        // Overwrite only non-empty blocks.
        let mut index = 0;

        let mask_acc =
            self.sets.clone()
            .map(|set|{
                let (level1_mask, valid) = set.update_level1_blocks3(
                    &mut *level1_blocks_ptr.add(index),
                    level0_index
                );
                index += valid as usize;
                level1_mask
            })
            .reduce(Op::hierarchy_op)
            .unwrap_unchecked();

        *level1_blocks_len = index;
        (mask_acc, index !=0)
    }

    #[inline]
    unsafe fn data_mask_from_blocks3(
        /*&self, */level1_blocks: &Self::Level1Blocks3, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock {
        unsafe{
            let slice = std::slice::from_raw_parts(
                level1_blocks.0.as_ptr(),
                level1_blocks.1
            );

            slice.iter()
                .map(|set_level1_blocks|
                    <S::Item as LevelMasksExt3>::data_mask_from_blocks3(
                        set_level1_blocks, level1_index
                    )
                )
                .reduce(Op::data_op)
                // level1_blocks can not be empty, since then -
                // level1 mask will be empty, and there will be nothing to iterate.
                .unwrap_unchecked()
        }
    }
}*/

impl<Op, S, Storage> LevelMasksRef for Reduce<Op, S, Storage>{}