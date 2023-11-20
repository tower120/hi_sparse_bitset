use std::mem;
use std::mem::{ManuallyDrop, size_of};
use crate::bit_op::{one_bits_iter, OneBitsIter};
use crate::Primitive;

#[inline]
fn mask_out<P: Primitive>(bit_block_iter: &mut OneBitsIter<P>, mask: P) {
    let block: &mut P = unsafe{
        mem::transmute(bit_block_iter)
    };
    *block &= mask;
}


/// Pop one bits. "Consumed" bits replaced with zero.
pub trait BitQueue: Iterator<Item = usize>{
    /// All bits 0. Iterator returns None.
    fn empty() -> Self;

    /// All bits 1.
    fn filled() -> Self;

    type Mask;

    /// Lower bits with 0 in mask.
    ///
    /// N.B. Bits with 1 in mask will **NOT** be raised.
    ///
    /// # Safety
    ///
    /// Panics, if mask size does not match BitQueue.
    ///
    /// _P.S. This should be compile-time panic
    /// (it is not due to RUST limitations). It does assert
    /// in compile-time, but throw panic runtime. So, kinda noop,
    /// when OK._
    fn mask_out(&mut self, mask: &Self::Mask);
}

pub struct PrimitiveBitQueue<P>{
    bit_block_iter: OneBitsIter<P>
}

impl<P> PrimitiveBitQueue<P>{
    #[inline]
    pub fn new(value: P) -> Self {
        Self{
            bit_block_iter: one_bits_iter(value)
        }
    }
}

impl<P> BitQueue for PrimitiveBitQueue<P>
where
    P: Primitive
{
    #[inline]
    fn empty() -> Self {
        Self::new(P::zero())
    }

    #[inline]
    fn filled() -> Self {
        Self::new(P::max_value())
    }

    type Mask = [P; 1];

    #[inline]
    fn mask_out(&mut self, mask: &[P; 1]) {
        mask_out(&mut self.bit_block_iter, mask[0]);
    }
}


impl<P> Iterator for PrimitiveBitQueue<P>
where
    P: Primitive
{
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.bit_block_iter.next()
    }
}

pub struct ArrayBitQueue<P, const N: usize>{
    bit_block_iters: [OneBitsIter<P>; N],
    bit_block_index: usize,
}

impl<P, const N: usize> ArrayBitQueue<P, N>
where
    P: Primitive
{
    #[inline]
    pub fn new(array: [P;N]) -> Self{
        Self{
            bit_block_iters: unsafe{
                // transmute is safe since OneBitsIter<P> transparent to P.
                // Should be just mem::transmute(array).
                mem::transmute_copy(&ManuallyDrop::new(array))
            },
            bit_block_index: 0,
        }
    }
}

impl<P, const N: usize> BitQueue for ArrayBitQueue<P, N>
where
    P: Primitive
{
    #[inline]
    fn empty() -> Self {
        Self{
            bit_block_iters: [one_bits_iter(P::zero()); N],
            bit_block_index: N-1,
        }
    }

    #[inline]
    fn filled() -> Self {
        Self::new([P::max_value(); N])
    }

    type Mask = [P; N];

    #[inline]
    fn mask_out(&mut self, mask: &[P; N]) {
        // update active one
        if self.bit_block_index == 0 {
            mask_out(&mut self.bit_block_iters[0], mask[0]);
        }

        // compile-time loop
        for i in 1..N {
            mask_out(&mut self.bit_block_iters[i], mask[i]);
        }
    }
}

impl<P, const N: usize> Iterator for ArrayBitQueue<P, N>
where
    P: Primitive
{
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(index) = self.bit_block_iters[0].next() {
                return Some(self.bit_block_index * size_of::<P>() * 8 + index);
            }
            if self.bit_block_index == N-1 {
                return None;
            }
            self.bit_block_index += 1;

            self.bit_block_iters[0] = unsafe {
                *self.bit_block_iters.get_unchecked_mut(self.bit_block_index)
            };
        }
    }
}