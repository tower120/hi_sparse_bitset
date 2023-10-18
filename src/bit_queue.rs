use std::mem;
use std::mem::{ManuallyDrop, size_of};
use crate::bit_block::BitBlock;
use crate::bit_op::{one_bits_iter, OneBitsIter};
use crate::MyPrimitive;

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
    fn mask_out(&mut self, mask: &Self::Mask);
}

// Rename to U64BitQueue
pub struct PrimitiveBitQueue{
    bit_block_iter: OneBitsIter<u64>
}

impl PrimitiveBitQueue{
    #[inline]
    pub fn new(value: u64) -> Self {
        Self{
            bit_block_iter: one_bits_iter(value)
        }
    }
}

impl BitQueue for PrimitiveBitQueue{
    #[inline]
    fn empty() -> Self {
        Self::new(0)
    }

    #[inline]
    fn filled() -> Self {
        Self::new(u64::MAX)
    }

    type Mask = [u64; 1];

    #[inline]
    fn mask_out(&mut self, mask: &[u64; 1]) {
        let mask = mask[0];
        let block: &mut u64 = unsafe{
            mem::transmute(&mut self.bit_block_iter)
        };
        *block &= mask;
    }
}


impl Iterator for PrimitiveBitQueue {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.bit_block_iter.next()
    }
}

pub struct ArrayBitQueue<const N: usize>{
    bit_block_iters: [OneBitsIter<u64>; N],
    // TODO: try and bench precomputed u32/usize block_start_index
    bit_block_index: usize,
}

impl<const N: usize> ArrayBitQueue< N> {
    #[inline]
    pub fn new(array: [u64;N]) -> Self{
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

impl<const N: usize> BitQueue for ArrayBitQueue<N> {
    #[inline]
    fn empty() -> Self {
        Self{
            bit_block_iters: [one_bits_iter(0); N],
            bit_block_index: N-1,
        }
    }

    #[inline]
    fn filled() -> Self {
        Self::new([u64::MAX; N])
    }

    type Mask = [u64; N];

    #[inline]
    fn mask_out(&mut self, mask: &[u64; N]) {
        // compile-time loop
        for i in 0..N{
            let bit_block_iter = &mut self.bit_block_iters[i];
            let bit_block: &mut u64 = unsafe{
                mem::transmute(bit_block_iter)
            };
            *bit_block &= mask[i];
        }
    }
}

impl<const N: usize> Iterator for ArrayBitQueue<N> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let bit_block_iter = unsafe {
                self.bit_block_iters.get_unchecked_mut(self.bit_block_index)
            };

            if let Some(index) = bit_block_iter.next() {
                return Some(self.bit_block_index * size_of::<u64>() + index);
            }

            if self.bit_block_index == N {
                return None;
            }
            self.bit_block_index += 1;
        }
    }
}