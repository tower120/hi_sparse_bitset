use core::mem::size_of;
use core::ops::ControlFlow;
use crate::Primitive;

/// Block ordering undefined. But same as [get_array_bit].
///
/// # Safety
///
/// `index` validity is not checked.
#[inline]
pub unsafe fn set_array_bit_unchecked<const FLAG: bool, T>(blocks: &mut [T], index: usize) -> bool
where
    T: Primitive
{
    let bits_size: usize = size_of::<T>() * 8;      // compile-time known value
    let block_index = index / bits_size;

    // index % size
    // From https://stackoverflow.com/a/27589182
    let bit_index = index & (bits_size -1);

    set_bit_unchecked::<FLAG, T>(blocks.get_unchecked_mut(block_index), bit_index)
}

/// In machine endian.
///
/// # Safety
///
/// `bit_index` validity is not checked.
#[inline]
pub unsafe fn set_bit_unchecked<const FLAG: bool, T>(block: &mut T, bit_index: usize) -> bool
where
    T: Primitive
{
    let block_mask: T = T::ONE << bit_index;
    let masked_block = *block & block_mask;

    if FLAG {
        *block |= block_mask;
    } else {
        *block &= !block_mask;
    }

    !masked_block.is_zero()
}

/// Block ordering undefined. But same as [set_array_bit].
///
/// # Safety
///
/// `index` validity is not checked.
#[inline]
pub unsafe fn get_array_bit_unchecked<T>(blocks: &[T], index: usize) -> bool
where
    T: Primitive
{
    let bits_size: usize = size_of::<T>() * 8;      // compile-time known value
    let block_index = index / bits_size;

    // index % size
    // From https://stackoverflow.com/a/27589182
    let bit_index = index & (bits_size -1);

    get_bit_unchecked(*blocks.get_unchecked(block_index), bit_index)
}

/// In machine endian.
///
/// # Safety
///
/// `bit_index` validity is not checked.
#[inline]
pub unsafe fn get_bit_unchecked<T: Primitive>(block: T, bit_index: usize) -> bool {
    // This generates `bt` instruction on x86
    let block_mask: T = T::ONE << bit_index;
    let masked_block = block & block_mask;
    !masked_block.is_zero()
}

/// In machine endian.
///
/// # Safety
///
/// `bit_index` validity is not checked.
#[inline]
pub unsafe fn zero_high_bits_unchecked<T: Primitive>(block: T, bit_index: usize) -> T {
cfg_select! {
    target_feature = "bmi2" => {
        use core::any::TypeId;
        if TypeId::of::<T>() == TypeId::of::<u64>(){
            return Primitive::from_u64(
                core::arch::x86_64::_bzhi_u64(block.as_u64(), bit_index as u32)
            );
        } else if TypeId::of::<T>() == TypeId::of::<u32>(){
            return Primitive::from_u32(
                core::arch::x86_64::_bzhi_u32(block.as_u32(), bit_index as u32)
            );
        } else {
            todo!();
        }
    }
    _ => {
        let mask: T = !(T::MAX << bit_index);
        block & mask
    }
}
}

#[inline]
pub fn pop_front<P>(block: &mut P) -> Option<u32>
where
    P: Primitive
{
    if block.is_zero(){
        return None;
    }
    Some(unsafe{pop_front_unchecked(block)})
}

/// # Safety
///
/// block must be non empty.
#[inline]
pub unsafe fn pop_front_unchecked<P>(block: &mut P) -> u32
where
    P: Primitive
{
    let index = block.trailing_zeros();
    *block &= block.wrapping_sub(P::ONE);
    index
}

/// Blocks traversed in the same order as [set_array_bit], [get_array_bit].
#[inline]
pub fn traverse_array_one_bits<P, F, B>(array: &[P], mut f: F) -> ControlFlow<B>
where
    P: Primitive,
    F: FnMut(usize) -> ControlFlow<B>
{
    let len = array.len();
    for i in 0..len{
        let element = unsafe{*array.get_unchecked(i)};
        let control = traverse_one_bits(
            element,
            |r|{
                let index = i*size_of::<P>()*8 + r;
                f(index)
            }
        );
        if let Some(e) = control.break_value() {
            return ControlFlow::Break(e);
        }
    }
    ControlFlow::Continue(())
}

#[inline]
pub fn traverse_one_bits<P, F, B>(mut element: P, mut f: F) -> ControlFlow<B>
where
    P: Primitive,
    F: FnMut(usize) -> ControlFlow<B>
{
    while let Some(index) = pop_front(&mut element){
        let control = f(index as usize);
        if let Some(e) = control.break_value() {
            return ControlFlow::Break(e);
        }
    }
    ControlFlow::Continue(())
}

/// This is 15% slower then "traverse" version
#[inline]
pub fn one_bits_iter<P>(element: P) -> OneBitsIter<P> {
    OneBitsIter {element}
}

/// Can be safely casted to its original bit block type.
///
/// "Consumed"/iterated one bits replaced with zero.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct OneBitsIter<P>{
    element: P
}
impl<P> Iterator for OneBitsIter<P>
where
    P: Primitive,
{
    type Item = usize;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        pop_front(&mut self.element).map(|i| i as usize)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<P> ExactSizeIterator for OneBitsIter<P>
where
    P: Primitive
{
    #[inline]
    fn len(&self) -> usize {
        self.element.count_ones() as usize
    }
}