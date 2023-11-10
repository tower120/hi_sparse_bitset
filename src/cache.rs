use std::marker::PhantomData;
use std::mem::MaybeUninit;

pub trait CacheStorage<T>{
    fn as_ptr(&self) -> *const T;
    fn as_mut_ptr(&mut self) -> *mut T;
}

pub trait CacheStorageBuilder: Default + Clone{
    type Storage<T>: CacheStorage<T>;

    /// MAX - if not fixed
    const FIXED_CAPACITY: usize;

    fn build<T, S>(size_getter: S) -> Self::Storage<T>
    where
        S: FnMut() -> usize;
}

/// This is NOT the same as ArrayVec. It does not store len.
pub struct FixedCacheStorage<T, const N: usize>([MaybeUninit<T>; N]);
impl<T, const N: usize> CacheStorage<T> for FixedCacheStorage<T, N>{
    #[inline]
    fn as_ptr(&self) -> *const T {
        self.0.as_ptr() as *const T
    }

    #[inline]
    fn as_mut_ptr(&mut self) -> *mut T {
        self.0.as_mut_ptr() as *mut T
    }
}

/// Fixed capacity cache.
#[derive(Default, Clone)]
pub struct FixedCache<const N: usize>;
impl<const N: usize> CacheStorageBuilder for FixedCache<N>{
    type Storage<T> = FixedCacheStorage<T, N>;

    const FIXED_CAPACITY: usize = N;

    #[inline]
    fn build<T, S>(_: S) -> Self::Storage<T>
    where
        S: FnMut() -> usize
    {
        unsafe{ MaybeUninit::uninit().assume_init() }
    }
}


pub struct DynamicCacheStorage<T>(Box<[T]>);
impl<T> CacheStorage<T> for DynamicCacheStorage<T>{
    #[inline]
    fn as_ptr(&self) -> *const T {
        self.0.as_ptr()
    }

    #[inline]
    fn as_mut_ptr(&mut self) -> *mut T {
        self.0.as_mut_ptr()
    }
}

/// Dynamically allocated cache.
///
/// If number of sets are enormously large, and you can't just increase FixedCache
/// size - you need this.
#[derive(Default, Clone)]
pub struct DynamicCache;
impl CacheStorageBuilder for DynamicCache{
    type Storage<T> = DynamicCacheStorage<T>;

    const FIXED_CAPACITY: usize = usize::MAX;

    #[inline]
    fn build<T, S>(mut size_getter: S) -> Self::Storage<T>
    where
        S: FnMut() -> usize
    {
        let size = size_getter();

        // TODO: make somehow faster?
        let mut v: Vec<T> = Vec::with_capacity(size);
        unsafe{
            v.set_len(size);
            let boxed = v.into_boxed_slice();
            DynamicCacheStorage(boxed)
        }
    }
}


/// Act as simple iterator.
#[derive(Default, Clone, Copy)]
pub struct NoCache;
impl CacheStorageBuilder for NoCache{
    type Storage<T> = NoCacheStorage;
    const FIXED_CAPACITY: usize = 0;
    fn build<T, S>(size_getter: S) -> Self::Storage<T> where S: FnMut() -> usize {
        NoCacheStorage
    }
}

#[derive(Default, Clone)]
pub struct NoCacheStorage;
impl<T> CacheStorage<T> for NoCacheStorage{
    fn as_ptr(&self) -> *const T {
        unreachable!()
    }
    fn as_mut_ptr(&mut self) -> *mut T {
        unreachable!()
    }
}





pub trait ReduceStorage{
    const IS_NONE: bool;
    type Cache;
}