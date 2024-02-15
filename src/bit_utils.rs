use std::mem::size_of;
use std::ops::{ControlFlow, RangeFrom, RangeInclusive, RangeToInclusive};
use crate::Primitive;

/// Block ordering undefined. But same as [get_array_bit].
/// 
/// Returns (original_bit, edited_primitive)
/// 
/// # Safety
/// 
/// `index` validity is not checked.
#[inline]
pub unsafe fn set_array_bit_unchecked<const FLAG: bool, T>(blocks: &mut [T], index: usize) 
    -> (bool, T)
where
    T: Primitive
{
    let bits_size: usize = size_of::<T>() * 8;      // compile-time known value
    let block_index = index / bits_size;

    // index % size
    // From https://stackoverflow.com/a/27589182
    let bit_index = index & (bits_size -1);

    let block = blocks.get_unchecked_mut(block_index);
    let original = set_bit_unchecked::<FLAG, T>(block, bit_index);
    (original, *block)
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

// TODO: consider removing
// TODO: NOT FULLY TESTED
/// Element at split point mutated.
/// 
/// Direction: 0 - left; 1 - right;
/// 
#[inline]
pub unsafe fn split_array_bits_unchecked<const DIRECTION: usize, T: Primitive>(blocks: &mut [T], at: usize) -> (usize, &mut [T]) {
    let element_index = at / (size_of::<T>() * 8); // compile-time math optimization
    let bit_index     = at % (size_of::<T>() * 8); // compile-time math optimization
    
    let block = blocks.get_unchecked_mut(element_index);
    match DIRECTION{
        0 /*left*/ => {
            *block &= !(T::MAX << bit_index);
            
            let slice = &mut*std::ptr::slice_from_raw_parts_mut(
                blocks.as_mut_ptr(), element_index+1
            );
            (0, slice)
        },
        1 /*right*/ => {
            *block &= T::MAX << bit_index;
            
            let slice = &mut*std::ptr::slice_from_raw_parts_mut(
                block, blocks.len() - element_index
            );
            (element_index * size_of::<T>() * 8, slice)
        },
        _ => panic!() 
    }    
}

/// `blocks` will be mutated.
/// 
/// # Safety
/// 
/// * `range` must be in `blocks` range.
#[inline]
pub unsafe fn slice_array_bits_unchecked<T: Primitive>(blocks: &mut [T], range: RangeInclusive<usize>) -> (usize, &mut [T]) {
    let (range_first, range_last) = range.into_inner();

    let first_element_index = range_first / (size_of::<T>() * 8); // compile-time math optimization
    let last_element_index  = range_last  / (size_of::<T>() * 8); // compile-time math optimization

    let first_bit_index = range_first % (size_of::<T>() * 8); // compile-time math optimization
    let last_bit_index  = range_last  % (size_of::<T>() * 8); // compile-time math optimization

    let first_block = blocks.get_unchecked_mut(first_element_index);
    *first_block &= T::MAX << first_bit_index;

    let last_block = blocks.get_unchecked_mut(last_element_index);
    *last_block &= !((T::MAX - T::ONE) << last_bit_index);  // !(T::MAX << (last_bit_index-1)) 

    let slice = &mut*std::ptr::slice_from_raw_parts_mut(
        blocks.as_mut_ptr().add(first_element_index), 1 + last_element_index - first_element_index
    );
    (first_element_index*size_of::<T>()*8, slice)
}

/// # Safety
/// 
/// * `n` must be in `blocks` bit-range.
/// * `blocks` must be non-empty.
#[inline]
pub unsafe fn fill_array_bits_to_unchecked<const FLAG: bool, T: Primitive>(blocks: &mut [T], range: RangeToInclusive<usize>) {
    debug_assert!(!blocks.is_empty());
    let last = range.end + 1;
    let element_index = last / (size_of::<T>() * 8); // compile-time math optimization
    let bit_index     = last % (size_of::<T>() * 8); // compile-time math optimization
    
    // skip last element on fill
    let first_part = &mut*std::ptr::slice_from_raw_parts_mut(
        blocks.as_mut_ptr(), element_index
    );
    let block = blocks.get_unchecked_mut(element_index);
    let mask = T::MAX << bit_index;
    if FLAG {
        first_part.fill(T::MAX);
        *block |= !mask;
    } else {
        first_part.fill(T::ZERO);
        *block &= mask;
    }
}

/// # Safety
/// 
/// * `n` must be in `blocks` bit-range.
/// * `blocks` must be non-empty.
#[inline]
pub unsafe fn fill_array_bits_from_unchecked<const FLAG: bool, T: Primitive>(blocks: &mut [T], range: RangeFrom<usize>) {
    debug_assert!(!blocks.is_empty());
    let element_index = range.start / (size_of::<T>() * 8); // compile-time math optimization
    let bit_index     = range.start % (size_of::<T>() * 8); // compile-time math optimization
    
    // skip first element on fill
    let start_fill_index = element_index + 1;
    let slice_to_fill = &mut*std::ptr::slice_from_raw_parts_mut(
        blocks.as_mut_ptr().add(start_fill_index), blocks.len() - start_fill_index 
    );
    
    let block = blocks.get_unchecked_mut(element_index);
    let mask = !(T::MAX << bit_index);
    if FLAG {
        slice_to_fill.fill(T::MAX);
        *block |= !mask;
    } else {
        slice_to_fill.fill(T::ZERO);
        *block &= mask;
    }
}

/// # Safety
/// 
/// `range` must be in `blocks` bit-range.
#[inline]
pub unsafe fn fill_array_bits_unchecked<const FLAG: bool, T: Primitive>(blocks: &mut [T], range: RangeInclusive<usize>) {
    let (range_first, range_last) = range.into_inner();

    let first_element_index = range_first / (size_of::<T>() * 8); // compile-time math optimization
    let first_bit_index     = range_first % (size_of::<T>() * 8); // compile-time math optimization
    
    let range_last = range_last;
    let last_element_index = range_last / (size_of::<T>() * 8); // compile-time math optimization
    let last_bit_index     = range_last % (size_of::<T>() * 8); // compile-time math optimization
    
    let left_mask  = T::MAX << first_bit_index;
    let right_mask = !((T::MAX - T::ONE) << last_bit_index);    // same as !(T::MAX << (last_bit_index+1)), considering shift overflow == 0.
    
    if first_element_index == last_element_index {
        let mask = left_mask & right_mask;
        let block = blocks.get_unchecked_mut(first_element_index); 
        if FLAG {
            *block |= mask;
        } else {
            *block &= !mask;
        }        
    } else {
        // skip first and last element on fill
        let first_solid_index = first_element_index + 1 as usize;
        // Equals to:
        // last_solid_index = last_element_index - 1
        // solid_blocks_len = last_solid_index - first_solid_index + 1
        let solid_blocks_len  = last_element_index - first_solid_index;
        let solid_blocks = &mut*std::ptr::slice_from_raw_parts_mut(
          blocks.as_mut_ptr().add(first_solid_index), solid_blocks_len
        );
        solid_blocks.fill(
            if FLAG {T::MAX} else {T::ZERO}
        );
        
        if FLAG {
            *blocks.get_unchecked_mut(first_element_index) |= left_mask;
            *blocks.get_unchecked_mut(last_element_index)  |= right_mask;
        } else {
            *blocks.get_unchecked_mut(first_element_index) &= !left_mask;
            *blocks.get_unchecked_mut(last_element_index)  &= !right_mask;
        }        
    }
}

// TODO: traverse_one_bits_array
//
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
        // TODO: benchmark this change (should be identical)
        let start_index = i*size_of::<P>()*8;
        let control = traverse_one_bits(
            element,
            |r|{
                let index = start_index + r;
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

// TODO: one_bits_array_iter ?
#[inline]
pub fn array_one_bits_iter<I>(blocks: I) -> ArrayOneBitsIter<I::IntoIter>
where
    I: IntoIterator,
    I::Item: Primitive
{
    let mut blocks_iter = blocks.into_iter();
    let block = blocks_iter.next().unwrap_or(Primitive::ZERO);
    
    ArrayOneBitsIter { 
        start_index: 0, 
        blocks_iter, 
        bit_iter: one_bits_iter(block)
    }
}

pub struct ArrayOneBitsIter<I>
where
    I: Iterator,
    I::Item: Primitive
{
    start_index: usize,
    blocks_iter: I,
    bit_iter: OneBitsIter<I::Item>
}

impl<I> Iterator for ArrayOneBitsIter<I>
where
    I: Iterator,
    I::Item: Primitive
{
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop{
            if let Some(value) = self.bit_iter.next(){
                return Some(self.start_index + value);
            } else {
                if let Some(block) = self.blocks_iter.next(){
                    self.bit_iter = one_bits_iter(block);
                } else {
                    return None;
                } 
                self.start_index += size_of::<I::Item>() * 8;
            }
        }
    }
}


#[cfg(test)]
mod test{
    use super::*;
    use itertools::assert_equal;
    
    #[test]
    fn test_split(){
        unsafe{
            let mut n = [0u64];
            set_array_bit_unchecked::<true, _>(&mut n, 1);
            set_array_bit_unchecked::<true, _>(&mut n, 3);
            set_array_bit_unchecked::<true, _>(&mut n, 4);
            set_array_bit_unchecked::<true, _>(&mut n, 10);
            assert_equal(array_one_bits_iter(n), [1,3,4,10]);
            
            {
                let mut n = n.clone();
                let s = split_array_bits_unchecked::<0, _>(&mut n, 4);
                assert_eq!(s.0, 0);
                assert_equal(array_one_bits_iter(s.1.iter().copied()), [1,3]);
            }
            {
                let mut n = n.clone();
                let s = split_array_bits_unchecked::<1, _>(&mut n, 4);
                assert_eq!(s.0, 0);
                assert_equal(array_one_bits_iter(s.1.iter().copied()), [4,10]);
            }
        }
    }

    #[test]
    fn test_slice(){        
        unsafe{
            let mut n = [0u64];
            set_array_bit_unchecked::<true, _>(&mut n, 1);
            set_array_bit_unchecked::<true, _>(&mut n, 3);
            set_array_bit_unchecked::<true, _>(&mut n, 4);
            set_array_bit_unchecked::<true, _>(&mut n, 10);
            set_array_bit_unchecked::<true, _>(&mut n, 11);
            set_array_bit_unchecked::<true, _>(&mut n, 12);
            assert_equal(array_one_bits_iter(n), [1,3,4,10,11,12]);
            
            let s = slice_array_bits_unchecked(&mut n, 3..=11);
            assert_eq!(s.0, 0);
            assert_equal(array_one_bits_iter(s.1.iter().copied()), [3,4,10,11]);
        }        
        
        unsafe{
            let mut n = [0u64;4];
            set_array_bit_unchecked::<true, _>(&mut n, 1);
            set_array_bit_unchecked::<true, _>(&mut n, 3);
            set_array_bit_unchecked::<true, _>(&mut n, 4);
            set_array_bit_unchecked::<true, _>(&mut n, 62);
            set_array_bit_unchecked::<true, _>(&mut n, 63);
            set_array_bit_unchecked::<true, _>(&mut n, 64);
            set_array_bit_unchecked::<true, _>(&mut n, 65);
            set_array_bit_unchecked::<true, _>(&mut n, 66);
            assert_equal(array_one_bits_iter(n), [1,3,4,62,63,64,65,66]);

            {
                let mut n = n.clone();
                let s = slice_array_bits_unchecked(&mut n, 3..=63);
                assert_eq!(s.0, 0);
                assert_equal(array_one_bits_iter(s.1.iter().copied()), [3,4,62,63]);
            }
            {
                let mut n = n.clone();
                let s = slice_array_bits_unchecked(&mut n, 3..=64);
                assert_eq!(s.0, 0);
                assert_equal(array_one_bits_iter(s.1.iter().copied()), [3,4,62,63,64]);
            }
        }
    }

    #[test]
    fn test_fill_first(){
        let mut n = [0u64; 2];
        unsafe{
            set_array_bit_unchecked::<true, _>(&mut n, 1);
            set_array_bit_unchecked::<true, _>(&mut n, 3);
            set_array_bit_unchecked::<true, _>(&mut n, 4);
            set_array_bit_unchecked::<true, _>(&mut n, 10);
            assert_equal(one_bits_iter(n[0]), [1,3,4,10]);   
            
            fill_array_bits_to_unchecked::<false, _>(&mut n, ..=3);
            assert_equal(one_bits_iter(n[0]), [4,10]);
            
            fill_array_bits_to_unchecked::<true, _>(&mut n, ..=5);
            assert_equal(one_bits_iter(n[0]), [0,1,2,3,4,5,10]);

            fill_array_bits_to_unchecked::<true, _>(&mut n, ..=63);
            assert_equal(one_bits_iter(n[0]), 0..=63);

            fill_array_bits_to_unchecked::<true, _>(&mut n, ..=64);
            assert_equal(array_one_bits_iter(n), 0..=64);
        }         
    }
    
    #[test]
    fn test_fill_last(){
        let mut n = [0u64];
        unsafe{
            set_array_bit_unchecked::<true, _>(&mut n, 1);
            set_array_bit_unchecked::<true, _>(&mut n, 3);
            set_array_bit_unchecked::<true, _>(&mut n, 4);
            set_array_bit_unchecked::<true, _>(&mut n, 60);
            set_array_bit_unchecked::<true, _>(&mut n, 61);
            set_array_bit_unchecked::<true, _>(&mut n, 63);
            assert_equal(one_bits_iter(n[0]), [1,3,4,60,61,63]);   
            
            fill_array_bits_from_unchecked::<false, _>(&mut n, 61..);
            assert_equal(one_bits_iter(n[0]), [1,3,4, 60]);
            
            fill_array_bits_from_unchecked::<true, _>(&mut n, 60..);
            assert_equal(one_bits_iter(n[0]), [1,3,4,60,61,62,63]);
        }         
    }    
    
    #[test]
    fn test_fill_range(){
        // insert
        unsafe{
            let mut n = [0u64];
            let range = 15..=58;
            fill_array_bits_unchecked::<true, _>(&mut n, range.clone());
            assert_equal(array_one_bits_iter(n), range.clone());
        }
        // reset
        unsafe{
            let mut n = [u64::MAX];
            fill_array_bits_unchecked::<false, _>(&mut n, 15..=57);
            assert_equal(array_one_bits_iter(n), (0..15).chain(58..64));
        }
        
        // insert array
        unsafe{
            let mut n = [0u64; 4];
            let range = 15..=203;
            fill_array_bits_unchecked::<true, _>(&mut n, range.clone());
            assert_equal(array_one_bits_iter(n), range.clone());
        }
        unsafe{
            let mut n = [0u64; 4];
            let range = 15..=68;
            fill_array_bits_unchecked::<true, _>(&mut n, range.clone());
            assert_equal(array_one_bits_iter(n), range.clone());
        }        
        
        // remove array
        unsafe{
            let mut n = [u64::MAX; 4];
            let range = 15..=202;
            fill_array_bits_unchecked::<false, _>(&mut n, range.clone());
            assert_equal(array_one_bits_iter(n), (0..15).chain(203..256));
        }
        unsafe{
            let mut n = [u64::MAX; 4];
            let range = 15..=67;
            fill_array_bits_unchecked::<false, _>(&mut n, range.clone());
            assert_equal(array_one_bits_iter(n), (0..15).chain(68..256));
        }
    }
    
    #[test]
    fn test_fill_range_regression1(){
        unsafe{
            let mut n = [0u64];
            let range = 0..=63;
            fill_array_bits_unchecked::<true, _>(&mut n, range.clone());
            assert_equal(array_one_bits_iter(n), range.clone());
        }
    }
}