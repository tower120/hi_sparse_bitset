use std::mem::size_of;
use std::ops::ControlFlow;
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
    let block_mask: T = T::ONE << bit_index;
    let masked_block = block & block_mask;
    !masked_block.is_zero()
}

/// Blocks traversed in the same order as [set_array_bit], [get_array_bit].
#[inline]
pub fn traverse_array_one_bits<P, F>(array: &[P], mut f: F) -> ControlFlow<()>
where
    P: Primitive,
    F: FnMut(usize) -> ControlFlow<()>
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
        if control.is_break(){
            return ControlFlow::Break(());
        }
    }
    ControlFlow::Continue(())
}

#[inline]
pub fn traverse_one_bits<P, F>(mut element: P, mut f: F) -> ControlFlow<()>
where
    P: Primitive,
    F: FnMut(usize) -> ControlFlow<()>
{
    // from https://lemire.me/blog/2018/03/08/iterating-over-set-bits-quickly-simd-edition/
    // https://github.com/lemire/Code-used-on-Daniel-Lemire-s-blog/blob/master/2018/03/07/simdbitmapdecode.c#L45
    while !element.is_zero() {
        let index = element.trailing_zeros() as usize;

        let control = f(index);
        if control.is_break(){
            return ControlFlow::Break(());
        }

        // Returns an integer having just the least significant bit of
        // bitset turned on, all other bits are off.
        let t: P = element & element.wrapping_neg();

        element ^= t;
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
        // from https://lemire.me/blog/2018/03/08/iterating-over-set-bits-quickly-simd-edition/
        // https://github.com/lemire/Code-used-on-Daniel-Lemire-s-blog/blob/master/2018/03/07/simdbitmapdecode.c#L45
        if !self.element.is_zero() {
            let index = self.element.trailing_zeros() as usize;

            // Returns an integer having just the least significant bit of
            // bitset turned on, all other bits are off.
            let t: P = self.element & self.element.wrapping_neg();
            self.element ^= t;

            Some(index)
        } else {
            None
        }
    }
}

