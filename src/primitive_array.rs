use std::mem::MaybeUninit;
use crate::internals::Primitive;

pub trait PrimitiveArray: AsRef<[Self::Item]> + AsMut<[Self::Item]> + Copy{
    type Item: Primitive;
    const CAP: usize;
    
    type UninitArray: UninitPrimitiveArray<UninitItem = Self::Item>;
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
    //type Item? 
    type UninitItem: Primitive;
    const CAP: usize;
    
    fn uninit_array() -> Self;
}
impl<T, const N: usize> UninitPrimitiveArray for [MaybeUninit<T>; N]
where
    T: Primitive
{
    type UninitItem = T;
    const CAP: usize = N;
    
    #[inline]
    fn uninit_array() -> Self{
        // From Rust MaybeUninit::uninit_array() :
        // SAFETY: An uninitialized `[MaybeUninit<_>; LEN]` is valid.
        unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() }        
    }
}