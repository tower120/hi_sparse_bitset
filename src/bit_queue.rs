//! Bit iterator for [BitBlock]. 
//! 
//! [BitBlock]: crate::BitBlock

use std::mem;
use std::mem::{ManuallyDrop, size_of};
use std::ops::ControlFlow;
use std::ops::ControlFlow::Break;

use crate::bit_utils::{one_bits_iter, OneBitsIter, self};
use crate::Primitive;

/// Return 0 if n > BITS
/* #[inline]
fn saturating_shl(i: u64, n: u32) -> u64{
    let (res, overflow) = i.overflowing_shl(n);
    res * overflow as u64
} */
#[inline]
fn saturating_shl<P: Primitive>(p: P, n: usize) -> P {
    let bits = size_of::<P>() * 8;
    if n >= bits{
        P::zero()
    } else {
        p << n
    }
}

#[inline]
fn trailing_zeroes<P: Primitive>(bit_block_iter: &OneBitsIter<P>) -> usize{
    let block: &P = unsafe{
        mem::transmute(bit_block_iter)
    };
    block.trailing_zeros() as usize
}

/*#[inline]
fn is_empty<P: Primitive>(bit_block_iter: &OneBitsIter<P>) -> bool{
    let block: &P = unsafe{
        mem::transmute(bit_block_iter)
    };
    block.is_zero()
}*/

/// Queue of 1 bits.
/// 
/// Pop first set bit on iteration. "Consumed" bit replaced with zero.
pub trait BitQueue: Iterator<Item = usize> + Clone{
    /// All bits 0. Iterator returns None.
    fn empty() -> Self;

    /// All bits 1.
    fn filled() -> Self;

    /* /// Remove first n bits. (Set 0)
    /// 
    /// # Safety
    /// 
    /// n is not checked.
    unsafe fn zero_first_n_unchecked(&mut self, n: usize); */

    /// Remove first n bits. (Set 0)
    /// 
    /// If n >= BitQueue len - make it empty.
    fn zero_first_n(&mut self, n: usize);

    /// Current index. Equals len - if iteration finished.
    fn current(&self) -> usize;

    fn traverse<F>(self, f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>;        
    
/*    // TODO: remove ?
    fn is_empty(&self) -> bool;*/
}

/// [BitQueue] for [Primitive].
#[derive(Clone)]
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

    /* #[inline]
    unsafe fn zero_first_n_unchecked(&mut self, n: usize) {
        zero_first_n(&mut self.bit_block_iter, n);
    } */

    #[inline]
    fn zero_first_n(&mut self, n: usize) {
        let block: &mut P = unsafe{
            mem::transmute(&mut self.bit_block_iter)
        };
        let mask = saturating_shl(P::max_value(), n);
        *block &= mask;
    }

    #[inline]
    fn current(&self) -> usize {
        trailing_zeroes(&self.bit_block_iter)
    }

    #[inline]
    fn traverse<F>(self, f: F) -> ControlFlow<()> where F: FnMut(usize) -> ControlFlow<()> {
        let block: P = unsafe{
            mem::transmute_copy(&self.bit_block_iter)
        };
        bit_utils::traverse_one_bits(block, f)
    }

    /*fn is_empty(&self) -> bool {
        is_empty(&self.bit_block_iter)
    }*/
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

/// [BitQueue] for array of [Primitive]s.
#[derive(Clone)]
pub struct ArrayBitQueue<P, const N: usize>{
    /// first element - always active one. 
    /// (copy of bit_block_iters[bit_block_index]).
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

/*     #[inline]
    unsafe fn zero_first_n_unchecked(&mut self, n: usize) {
        let element_index = n / (size_of::<P>() * 8); // compile-time math optimization
        let bit_index     = n % (size_of::<P>() * 8); // compile-time math optimization

        // Fill all first elements with 0
        for i in 0..element_index{
            *self.bit_block_iters.get_unchecked_mut(i) = one_bits_iter(P::zero());
        }
        
        // Mask out last one
        zero_first_n(&mut self.bit_block_iters.get_unchecked_mut(element_index), bit_index);
    } */

    #[inline]
    fn zero_first_n(&mut self, n: usize) {
        let element_index = n / (size_of::<P>() * 8); // compile-time math optimization
        
        // clamp to empty
        if element_index >= N {
            //*self = Self::empty(); 
            self.bit_block_iters[0] = one_bits_iter(P::zero());
            self.bit_block_index = N-1;
            return;
        }
        
        // are we ahead of n block-wise? 
        if element_index < self.bit_block_index {
            return;
        }



/*        // 2.0
        unsafe {
            self.bit_block_index = element_index;
            
            let active_block_iter = unsafe {
                self.bit_block_iters.get_unchecked_mut(element_index)
            };
            
            // Mask out block                        
            let bit_index = n % (size_of::<P>() * 8); // compile-time math optimization
            unsafe /* zero_first_n */ {
                let block: &mut P = mem::transmute(active_block_iter);
                *block &= P::max_value() << bit_index;            
            }
            
            // copy to active
            self.bit_block_iters[0] = *active_block_iter;
            
            return;
        }*/
        
        
        // update active block
        if element_index != self.bit_block_index {
            self.bit_block_index = element_index;
            self.bit_block_iters[0] = unsafe {
                *self.bit_block_iters.get_unchecked_mut(element_index)
            };
        }

        // Mask out active block                        
        let bit_index = n % (size_of::<P>() * 8); // compile-time math optimization
        unsafe /* zero_first_n */ {
            let active_block_iter = &mut self.bit_block_iters[0];
            let block: &mut P = mem::transmute(active_block_iter);
            *block &= P::max_value() << bit_index;            
        }
    }

    #[inline]
    fn current(&self) -> usize {
        let active_block_iter = &self.bit_block_iters[0];
        self.bit_block_index * size_of::<P>() * 8 + trailing_zeroes(active_block_iter)
    }

    #[inline]
    fn traverse<F>(mut self, mut f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>        
    {
        // This is faster, then iterating active value, then the rest ones
        unsafe{
            // copy active back to its place.
            // compiler should optimize away this for newly constructed BitQueue.
            *self.bit_block_iters.get_unchecked_mut(self.bit_block_index) = self.bit_block_iters[0];
            
            let slice: &[P] = unsafe{std::slice::from_raw_parts(
                // cast is safe because OneBitsIter<P> transmutable to P.
                self.bit_block_iters.as_ptr().add(self.bit_block_index).cast(),
                N - self.bit_block_index
            )};
            // TODO: maybe this can be better (it is already multiplied inside traverse)
            let start_index = self.bit_block_index*size_of::<P>()*8;
            return bit_utils::traverse_array_one_bits( slice, |i|f(start_index + i));
        }
        
        /* // 1. Traverse active
        let block: P = unsafe{
            mem::transmute_copy(&self.bit_block_iters[0])
        };
        let start_index = self.bit_block_index*size_of::<P>()*8; 
        let ctrl = bit_utils::traverse_one_bits(block, |i|f(start_index + i));
        if ctrl.is_break(){
            return Break(());
        }
        
        // 2. Traverse the rest
        let from_index = self.bit_block_index + 1;
        let slice: &[P] = unsafe{std::slice::from_raw_parts(
            // cast is safe because OneBitsIter<P> transmutable to P.
            self.bit_block_iters.as_ptr().add(from_index).cast(),
            N-from_index
        )};
        // TODO: do better
        let start_index = from_index*size_of::<P>()*8; 
        bit_utils::traverse_array_one_bits( slice, |i|f(start_index+i)) */
    }


/*    #[inline]
    fn is_empty(&self) -> bool {
        let active_block_iter = &self.bit_block_iters[0];
        is_empty(active_block_iter)
    }*/
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

    #[inline]
    fn for_each<F>(self, mut f: F)
    where
        F: FnMut(usize)
    {
        self.traverse(|i|{
            f(i);
            ControlFlow::Continue(())
        });
    }
}