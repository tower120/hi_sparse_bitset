use std::mem::MaybeUninit;
use crate::internals::Primitive;

pub trait PrimitiveArray: AsRef<[Self::Item]> + AsMut<[Self::Item]> + Copy{
    type Item: Primitive;
    const CAP: usize;
    
    type UninitArray: UninitPrimitiveArray<InitArray = Self, UninitItem = Self::Item>;
    #[inline]
    fn uninit() -> Self::UninitArray {
        Self::UninitArray::new()
    }
}
impl<T, const N: usize> PrimitiveArray for [T; N]
where
    T: Primitive
{
    type Item = T;
    const CAP: usize = N;
    type UninitArray = [MaybeUninit<Self::Item>; N];
}

#[allow(dead_code)]     // Because not publicly visibile
pub trait UninitPrimitiveArray
    : AsRef<[MaybeUninit<Self::UninitItem>]> 
    + AsMut<[MaybeUninit<Self::UninitItem>]> 
    + Copy
{
    type InitArray: PrimitiveArray;
    
    //type Item? 
    type UninitItem: Primitive;
    const CAP: usize;
    
    fn new() -> Self;
    
    #[inline]
    fn assume_init(self) -> Self::InitArray {
        unsafe { std::mem::transmute_copy(&self) }
    }    
}
impl<T, const N: usize> UninitPrimitiveArray for [MaybeUninit<T>; N]
where
    T: Primitive
{
    type InitArray = [T; N];
    type UninitItem = T;
    const CAP: usize = N;
    
    #[inline]
    fn new() -> Self{
        // From Rust MaybeUninit::uninit_array() :
        // SAFETY: An uninitialized `[MaybeUninit<_>; LEN]` is valid.
        unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() }        
    }
}