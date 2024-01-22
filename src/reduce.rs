use std::marker::PhantomData;
use std::{mem, ptr};
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ptr::NonNull;
use crate::{assume, BitSetInterface, LevelMasks};
use crate::implement::impl_bitset;
use crate::ops::BitSetOp;
use crate::cache::ReduceCache;
use crate::bitset_interface::{BitSetBase, LevelMasksIterExt};
use crate::config::Config;

/// Bitsets iterator reduction, as lazy bitset.
///
/// Constructed by [reduce] and [reduce_w_cache].
/// 
/// [reduce]: crate::reduce()
/// [reduce_w_cache]: crate::reduce_w_cache()
#[derive(Clone)]
#[repr(transparent)]
pub struct Reduce<Op, S, Cache> {
    pub(crate) sets: S,
    pub(crate) phantom: PhantomData<(Op, Cache)>
}

impl<Op, S, Cache> BitSetBase for Reduce<Op, S, Cache>
where
    Op: BitSetOp,
    S: Iterator + Clone,
    S::Item: LevelMasks
{
    type Conf = <S::Item as BitSetBase>::Conf;

    /// true if S and Op are `TrustedHierarchy`.
    const TRUSTED_HIERARCHY: bool = Op::TRUSTED_HIERARCHY & S::Item::TRUSTED_HIERARCHY;
}

impl<Op, S, Cache> LevelMasks for Reduce<Op, S, Cache>
where
    Op: BitSetOp,
    S: Iterator + Clone,
    S::Item: LevelMasks
{
    #[inline]
    fn level0_mask(&self) -> <Self::Conf as Config>::Level0BitBlock {
        unsafe{
            self.sets.clone()
            .map(|set| set.level0_mask())
            .reduce(Op::hierarchy_op)
            .unwrap_unchecked()
        }
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize)
        -> <Self::Conf as Config>::Level1BitBlock
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
        -> <Self::Conf as Config>::DataBitBlock
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
    type Conf: Config;
    type Set: LevelMasksIterExt<Conf = Self::Conf>;
    type Sets: Iterator<Item = Self::Set> + Clone;

    /// Cache only used by DynamicCache
    type IterState;
    fn make_state(sets: &Self::Sets) -> Self::IterState;
    fn drop_state(sets: &Self::Sets, state: &mut ManuallyDrop<Self::IterState>);

    type Level1BlockData: Default;
    unsafe fn init_level1_block_data(
        sets: &Self::Sets,
        state: &mut Self::IterState,
        level1_block_data: &mut MaybeUninit<Self::Level1BlockData>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool);
    unsafe fn data_mask_from_block_data(
        level1_blocks: &Self::Level1BlockData, level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock;
}

pub struct NonCachedImpl<Op, T>(PhantomData<(Op, T)>);
impl<Op, S> ReduceCacheImpl for NonCachedImpl<Op, S>
where
    Op: BitSetOp,
    S: Iterator + Clone,
    S::Item: LevelMasksIterExt,
{
    type Conf = <S::Item as BitSetBase>::Conf;
    type Set  = S::Item;
    type Sets = S;
    type IterState = ();
    type Level1BlockData = (Option<S>, usize);

    #[inline]
    fn make_state(_: &Self::Sets) -> Self::IterState { () }

    #[inline]
    fn drop_state(_: &Self::Sets, _: &mut ManuallyDrop<Self::IterState>) {}

    #[inline]
    unsafe fn init_level1_block_data(
        sets: &Self::Sets,
        _: &mut Self::IterState,
        level1_blocks: &mut MaybeUninit<Self::Level1BlockData>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        level1_blocks.write((Some(sets.clone()), level0_index));

        let reduce: &Reduce<Op, S, ()> = mem::transmute(sets);
        (reduce.level1_mask(level0_index), true)
    }

    #[inline]
    unsafe fn data_mask_from_block_data(
        level1_blocks: &Self::Level1BlockData, level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock {
        let (sets, level0_index) = level1_blocks;

        let reduce: &Reduce<Op, S, ()> = mem::transmute(sets.as_ref().unwrap_unchecked());
        reduce.data_mask(*level0_index, level1_index)
    }
}

#[inline(always)]
unsafe fn init_level1_block_data<Op, Conf, Sets>(
    _: Op,
    sets: &Sets,
    state_ptr: *mut <Sets::Item as LevelMasksIterExt>::IterState,
    level1_block_data_array_ptr: *mut MaybeUninit<<Sets::Item as LevelMasksIterExt>::Level1BlockData>,
    level0_index: usize
) -> (<Conf as Config>::Level1BitBlock, usize/*len*/, bool/*is_not_empty*/)
where
    Op: BitSetOp,
    Conf: Config,
    Sets: Iterator + Clone,
    Sets::Item: LevelMasksIterExt<Conf=Conf>,
{
    // intersection case can be optimized, since we know
    // that with intersection, there can be no
    // empty masks/blocks queried.
    //
    // P.S. should be const, but act as const anyway.
    /*const*/ let never_empty = Op::HIERARCHY_OPERANDS_CONTAIN_RESULT;

    // Overwrite only non-empty blocks.
    let mut state_index = 0;
    let mut index = 0;
    let mask =
        sets.clone()
        .map(|set|{
            let (level1_mask, is_not_empty) = set.init_level1_block_data(
                &mut *state_ptr.add(state_index),
                &mut *level1_block_data_array_ptr.add(index),
                level0_index
            );

            if never_empty {
                assume!(is_not_empty);
                index += 1;
                state_index = index;
            } else {
                index += is_not_empty as usize;
                state_index += 1;
            }

            level1_mask
        })
        .reduce(Op::hierarchy_op)
        .unwrap_unchecked();

    let is_not_empty =
        if never_empty {
            assume!(index != 0);
            true
        } else {
            index!=0
        };

    (mask, index, is_not_empty)
}

#[inline]
unsafe fn data_mask_from_block_data<Op, Set>(
    //_: Op,
    slice: &[Set::Level1BlockData],
    level1_index: usize
) -> <Set::Conf as Config>::DataBitBlock
where
    Op: BitSetOp,
    Set: LevelMasksIterExt,
{
    unsafe{
        let res = slice.iter()
            .map(|set_level1_blocks|
                <Set as LevelMasksIterExt>::data_mask_from_block_data(
                    set_level1_blocks, level1_index
                )
            )
            .reduce(Op::data_op);
        
        if Op::HIERARCHY_OPERANDS_CONTAIN_RESULT {
            // level1_blocks can not be empty, since then -
            // level1 mask will be empty, and there will be nothing to iterate.
            res.unwrap_unchecked()
        } else {
            res.unwrap_or_else(||<<Set::Conf as Config>::DataBitBlock as crate::BitBlock>::zero())
        }
    }
}

#[inline]
unsafe fn construct_child_state<Sets>(
    sets: &Sets,
    state_ptr: *mut MaybeUninit<<Sets::Item as LevelMasksIterExt>::IterState>
)
where
    Sets: Iterator + Clone,
    Sets::Item: LevelMasksIterExt
{
    let mut element_ptr = state_ptr;
    for set in sets.clone(){
        (*element_ptr).write(set.make_iter_state());
        element_ptr = element_ptr.add(1);
    }
}

#[inline]
unsafe fn destruct_child_state<Sets>(
    sets: &Sets,
    state_ptr: *mut ManuallyDrop<<Sets::Item as LevelMasksIterExt>::IterState>
)
where
    Sets: Iterator + Clone,
    Sets::Item: LevelMasksIterExt
{
    let mut element_ptr = state_ptr;
    for set in sets.clone(){
        set.drop_iter_state(&mut *element_ptr);
        element_ptr = element_ptr.add(1);
    }
}

/// ala ArrayVec
pub struct RawArray<T, const N: usize>{
    mem: [MaybeUninit<T>; N],
    len: usize
}
impl<T, const N: usize> Default for RawArray<T, N>{
    #[inline]
    fn default() -> Self {
        unsafe{
            Self{mem: MaybeUninit::uninit().assume_init(), len: 0}    
        }
    }
}
impl <T, const N: usize> Drop for RawArray<T, N>{
    #[inline]
    fn drop(&mut self) {
        if mem::needs_drop::<T>(){
            unsafe{
                let slice = std::slice::from_raw_parts_mut(self.mem.as_mut_ptr(), self.len);
                ptr::drop_in_place(slice);
            }
        }
    }
}

pub struct FixedCacheImpl<Op, S, const N: usize>(PhantomData<(Op, S)>)
where
    Op: BitSetOp,
    S: Iterator + Clone,
    S::Item: LevelMasksIterExt;

impl<Op, S, const N: usize> ReduceCacheImpl for FixedCacheImpl<Op, S, N>
where
    Op: BitSetOp,
    S: Iterator + Clone,
    S::Item: LevelMasksIterExt,
{
    type Conf = <S::Item as BitSetBase>::Conf;
    type Set = S::Item;
    type Sets = S;

    /// We use Level1Blocks directly, but childs may have data.
    /// Will be ZST, if no-one use. size = sets.len().
    type IterState = [MaybeUninit<<Self::Set as LevelMasksIterExt>::IterState>; N];

    /// Never drop, since array contain primitives, or array of primitives.
    type Level1BlockData = RawArray<<Self::Set as LevelMasksIterExt>::Level1BlockData, N>;

    #[inline]
    fn make_state(sets: &Self::Sets) -> Self::IterState {
        unsafe{
            let mut state = MaybeUninit::<Self::IterState>::uninit().assume_init();
            construct_child_state(sets, state.as_mut_ptr());
            mem::transmute(state)
        }
    }

    #[inline]
    fn drop_state(sets: &Self::Sets, state: &mut ManuallyDrop<Self::IterState>) {
        unsafe{
            destruct_child_state(sets, state.as_mut_ptr() as *mut _);
            ManuallyDrop::drop(state);
        }
    }

    #[inline]
    unsafe fn init_level1_block_data(
        sets: &Self::Sets,
        state: &mut Self::IterState,
        level1_blocks: &mut MaybeUninit<Self::Level1BlockData>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        let level1_blocks_storage = level1_blocks.assume_init_mut();
        // assume_init_mut() array
        let state_ptr = state.as_mut_ptr() as *mut <Self::Set as LevelMasksIterExt>::IterState;

        let (mask, len, valid) = init_level1_block_data(
            Op::default(),
            sets,
            state_ptr,
            level1_blocks_storage.mem.as_mut_ptr(),
            level0_index
        );
        level1_blocks_storage.len = len;
        (mask, valid)
    }

    #[inline]
    unsafe fn data_mask_from_block_data(
        level1_blocks: &Self::Level1BlockData, level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock {
        let slice = std::slice::from_raw_parts(
            level1_blocks.mem.as_ptr() as *const <Self::Set as LevelMasksIterExt>::Level1BlockData,
            level1_blocks.len
        );
        data_mask_from_block_data::<Op, Self::Set>(slice, level1_index)
    }
}

pub struct DynamicCacheImpl<Op, S>(PhantomData<(Op, S)>);
impl<Op, S> ReduceCacheImpl for DynamicCacheImpl<Op, S>
where
    Op: BitSetOp,
    S: Iterator + Clone,
    S::Item: LevelMasksIterExt
{
    type Conf =  <S::Item as BitSetBase>::Conf;
    type Set = S::Item;
    type Sets = S;

    /// Have two separate storages, to keep local storage tight, and fast to iterate
    type IterState = (
        Vec<<Self::Set as LevelMasksIterExt>::Level1BlockData>,

        // child state
        Box<[ManuallyDrop<<Self::Set as LevelMasksIterExt>::IterState>]>,
    );
    
    /// raw slice
    type Level1BlockData = (
        // This points to Self::IterState heap
        Option<NonNull<<Self::Set as LevelMasksIterExt>::Level1BlockData>>,
        usize
    );

    #[inline]
    fn make_state(sets: &Self::Sets) -> Self::IterState {
        let len = sets.clone().count();
        
        // Box::new_uninit_slice is still unsafe. 
        // We construct as UniqueArrayPtr, and then transfer ownership to Box<[]>.
        
        // 1. Allocate and initialize childs.
        let mut child_state = UniqueArrayPtr::new_uninit(len);
        unsafe{
            construct_child_state(sets, child_state.as_mut_ptr());
        }
        
        // 2. Transfer ownership to Box.
        let child_state = unsafe{
            let mut storage = ManuallyDrop::new(child_state);
            // cast UniqueArrayPtr<MaybeUninit<_>> -> UniqueArrayPtr<ManuallyDrop<_>>
            let storage_ptr = storage.as_mut_ptr() as *mut _;
            Box::from_raw(
                std::slice::from_raw_parts_mut(storage_ptr, len)
            )
        };

        (Vec::with_capacity(len), child_state)
    }

    #[inline]
    fn drop_state(sets: &Self::Sets, state: &mut ManuallyDrop<Self::IterState>) {
        unsafe{
            destruct_child_state(sets, state.1.as_mut_ptr());
            ManuallyDrop::drop(state);
        }
    }

    #[inline]
    unsafe fn init_level1_block_data(
        sets: &Self::Sets,
        state: &mut Self::IterState,
        level1_block_data: &mut MaybeUninit<Self::Level1BlockData>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        let (storage, child_state) = state;

        // &mut[ManuallyDrop<T>] -> &mut[T]
        let sets_state_ptr = child_state.as_mut_ptr() as *mut _;
        storage.clear();
        let level1_block_data_array_ptr = storage.spare_capacity_mut().as_mut_ptr();

        let (mask, len, valid) = init_level1_block_data(
            Op::default(),
            sets,
            sets_state_ptr,
            level1_block_data_array_ptr,
            level0_index
        );
        
        storage.set_len(len);

        level1_block_data.write((
            // assume_init_ref array
            Some(NonNull::new_unchecked(storage.as_mut_ptr())),
            len
        ));

        (mask, valid)
    }

    #[inline]
    unsafe fn data_mask_from_block_data(
        level1_blocks: &Self::Level1BlockData, level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock {
        let slice = std::slice::from_raw_parts(
            level1_blocks.0.unwrap_unchecked().as_ptr(),
            level1_blocks.1
        );
        data_mask_from_block_data::<Op, Self::Set>(slice, level1_index)
    }
}


impl<Op, S, Cache> LevelMasksIterExt for Reduce<Op, S, Cache>
where
    Op: BitSetOp,
    S: Iterator + Clone,
    S::Item: LevelMasksIterExt,
    Cache: ReduceCache
{
    type IterState = <Cache::Impl<Op, S> as ReduceCacheImpl>::IterState;
    type Level1BlockData = <Cache::Impl<Op, S> as ReduceCacheImpl>::Level1BlockData;

    #[inline]
    fn make_iter_state(&self) -> Self::IterState {
        <Cache::Impl<Op, S> as ReduceCacheImpl>::make_state(&self.sets)
    }

    #[inline]
    unsafe fn drop_iter_state(&self, state: &mut ManuallyDrop<Self::IterState>) {
        <Cache::Impl<Op, S> as ReduceCacheImpl>::drop_state(&self.sets, state)
    }

    #[inline]
    unsafe fn init_level1_block_data(
        &self,
        state: &mut Self::IterState,
        level1_block_data: &mut MaybeUninit<Self::Level1BlockData>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        <Cache::Impl<Op, S> as ReduceCacheImpl>::
            init_level1_block_data(&self.sets, state, level1_block_data, level0_index)
    }

    #[inline]
    unsafe fn data_mask_from_block_data(
        level1_blocks: &Self::Level1BlockData, level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock {
        <Cache::Impl<Op, S> as ReduceCacheImpl>::
            data_mask_from_block_data(level1_blocks, level1_index)
    }
}

impl_bitset!(
    impl<Op, S, Cache> for Reduce<Op, S, Cache>
    where
        Op: BitSetOp,
        S: Iterator + Clone,
        S::Item: BitSetInterface,
        Cache: ReduceCache
);

// Some methods not used by library.
#[allow(dead_code)]
mod unique_ptr{
    use std::alloc::{dealloc, Layout};
    use std::mem::MaybeUninit;
    use std::ptr::{drop_in_place, NonNull, null_mut};
    use std::{mem, slice};

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
}
use unique_ptr::UniqueArrayPtr;