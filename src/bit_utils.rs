use std::mem::size_of;
use std::ops::{BitAndAssign, BitOrAssign, BitXorAssign, ControlFlow};
use num_traits::int::PrimInt;
use num_traits::WrappingNeg;
use crate::Primitive;

/// Block ordering undefined. But same as [get_array_bit].
/// 
/// # Safety
/// 
/// `index` validity is not checked.
#[inline]
pub unsafe fn set_array_bit_unchecked<const FLAG: bool, T>(blocks: &mut [T], index: usize) -> bool
where
    T: PrimInt + BitAndAssign + BitOrAssign
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
    T: PrimInt + BitAndAssign + BitOrAssign
{
    let block_mask: T = T::one() << bit_index;
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
    T: PrimInt + BitAndAssign + BitOrAssign
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
pub unsafe fn get_bit_unchecked<T: PrimInt>(block: T, bit_index: usize) -> bool {
    let block_mask: T = T::one() << bit_index;
    let masked_block = block & block_mask;
    !masked_block.is_zero()
}

/// Blocks traversed in the same order as [set_array_bit], [get_array_bit].
#[inline]
pub fn traverse_array_one_bits<P, F>(array: &[P], mut f: F) -> ControlFlow<()>
where
    P: PrimInt + BitXorAssign + WrappingNeg,
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

//TODO: remove
/// Blocks traversed in the same order as [set_array_bit], [get_array_bit].
/// 
/// # Safety
/// 
/// index is not checked
#[inline]
pub unsafe fn traverse_array_one_bits_from_unchecked<P, F>(
    array: &[P], from_index: usize, mut f: F
) -> ControlFlow<()>
where
    P: PrimInt + BitXorAssign + BitAndAssign + WrappingNeg,
    F: FnMut(usize) -> ControlFlow<()>
{
    let mut traverse_block = |index: usize, element: P| -> ControlFlow<()> {
        let block_start_index = index * size_of::<P>() * 8; 
        traverse_one_bits(
            element,
            |r|{
                let index = block_start_index + r;
                f(index)
            }
        )        
    };
    
    let start_block_index = from_index / 8;
    let start_in_block_index = from_index % 8;  // CT?
    {
        let mut element = *array.get_unchecked(start_block_index);
        element &= P::max_value() << start_in_block_index;
        
        let control = traverse_block(start_block_index, element); 
        if control.is_break(){
            return ControlFlow::Break(());
        }
    }
    
    let len = array.len();     
    for i in start_block_index+1..len{
        let element = unsafe{*array.get_unchecked(i)};
        let control = traverse_block(i, element);
        if control.is_break(){
            return ControlFlow::Break(());
        }
    }
    ControlFlow::Continue(())
}

#[inline]
pub fn traverse_one_bits<P, F>(mut element: P, mut f: F) -> ControlFlow<()>
where
    P: PrimInt + BitXorAssign + WrappingNeg,
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
    P: PrimInt + BitXorAssign + WrappingNeg,
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

