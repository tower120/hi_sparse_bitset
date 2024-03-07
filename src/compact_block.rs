use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::{Deref, DerefMut};
use std::ops::ControlFlow::Continue;
use std::ptr;
use crate::BitBlock;
use crate::level::IBlock;
use crate::primitive::Primitive;
use crate::primitive_array::{PrimitiveArray, UninitPrimitiveArray};

#[repr(C)]
union BigSmallArray<BlockIndices, SmallBlockIndices, MaskU64Populations>
where
    BlockIndices: PrimitiveArray,
    SmallBlockIndices: PrimitiveArray<Item=BlockIndices::Item>,
    MaskU64Populations: PrimitiveArray<Item=u8>,
{
    big: (u8, ManuallyDrop<Box<BlockIndices>>),
    
    /// First element in `MaskU64Populations` always 0.
    /// 
    /// SmallBlockIndices len = MaskU64Populations.last() + mask.last().count_ones().  
    small: (MaskU64Populations, SmallBlockIndices::UninitArray)
}

impl<BlockIndices, SmallBlockIndices, MaskU64Populations> From<Box<BlockIndices>> for BigSmallArray<BlockIndices, SmallBlockIndices, MaskU64Populations>
where
    BlockIndices: PrimitiveArray,
    SmallBlockIndices: PrimitiveArray<Item=BlockIndices::Item>,
    MaskU64Populations: PrimitiveArray<Item=u8>
{
    #[inline]
    fn from(array: Box<BlockIndices>) -> Self {
        Self{
            big: (1, ManuallyDrop::new(array))
        }
    }
}

impl<BlockIndices, SmallBlockIndices, MaskU64Populations> From<(MaskU64Populations, SmallBlockIndices::UninitArray)> for BigSmallArray<BlockIndices, SmallBlockIndices, MaskU64Populations>
where
    BlockIndices: PrimitiveArray,
    SmallBlockIndices: PrimitiveArray<Item=BlockIndices::Item>,
    MaskU64Populations: PrimitiveArray<Item=u8>
{
    #[inline]
    fn from(small: (MaskU64Populations, SmallBlockIndices::UninitArray)) -> Self {
        debug_assert!(small.0.as_ref().first().unwrap().is_zero());
        Self{ small }
    }
}

impl<BlockIndices, SmallBlockIndices, MaskU64Populations> Clone for BigSmallArray<BlockIndices, SmallBlockIndices, MaskU64Populations>
where
    BlockIndices: PrimitiveArray,
    SmallBlockIndices: PrimitiveArray<Item=BlockIndices::Item>,
    MaskU64Populations: PrimitiveArray<Item=u8>
{
    #[inline]
    fn clone(&self) -> Self {
        unsafe{
            if self.is_big(){
                Self{big: (1, self.big.1.clone())}
            } else {
                Self{small: self.small}
            }
        }
    }
}

impl<BlockIndices, SmallBlockIndices, MaskU64Populations> BigSmallArray<BlockIndices, SmallBlockIndices, MaskU64Populations>
where
    BlockIndices: PrimitiveArray,
    SmallBlockIndices: PrimitiveArray<Item=BlockIndices::Item>,
    MaskU64Populations: PrimitiveArray<Item=u8>,
{
    #[inline]
    fn is_small(&self) -> bool {
        unsafe{ self.big.0 == 0 }
    }
    #[inline]
    fn is_big(&self) -> bool {
        !self.is_small()
    }
    
    /// number of 1 bits in mask before `index` bit.
    ///
    /// # Safety
    /// 
    /// `index` must be set
    #[inline]
    unsafe fn small_array_index<Mask: BitBlock>(mask_u64_populations: &MaskU64Populations, mask: &Mask, index: usize) 
        -> usize
    {
        let u64_index = index / 64;
        let bit_index = index % 64;
        let mut block = *mask.as_array().get_unchecked(u64_index);
        let mask = !(u64::MAX << bit_index);
        block &= mask;
        let offset = *mask_u64_populations.as_ref().get_unchecked(u64_index);
        offset as usize + block.count_ones() as usize
    }
    
    /*/// # Safety
    /// 
    /// `index` must exist/be set in block. 
    #[inline]
    unsafe fn get_unchecked<Mask: BitBlock>(&self, mask: &Mask, index: usize) -> BlockIndices::Item {
        match self{
            BigSmallArray::Big(array) => {
                unsafe { *array.deref().as_ref().get_unchecked(index) }
            }
            BigSmallArray::Small { mask_u64_populations, array, array_len } => {
                let small_array_index = Self::small_array_index(mask_u64_populations, mask, index);
                *array.as_ref().get_unchecked(small_array_index)
            }
        }
    }*/
    
    #[inline]
    unsafe fn get_or_zero<Mask: BitBlock>(&self, mask: &Mask, index: usize) -> BlockIndices::Item {
        if self.is_big(){
            let array = self.big.1.deref();
            *array.deref().as_ref().get_unchecked(index)
        } else {
            let (mask_u64_populations, array) = &self.small;
            let u64_index = index / 64;
            let bit_index = index % 64;
            let mut block = *mask.as_array().get_unchecked(u64_index);
            
            {
                let block_mask: u64 = 1 << bit_index;
                let masked_block = block & block_mask;
                if masked_block.is_zero(){
                    return Primitive::ZERO;
                }
            }
            
            let mask = !(u64::MAX << bit_index);
            block &= mask;
            
            let offset = *mask_u64_populations.as_ref().get_unchecked(u64_index);
            let small_array_index = offset as usize + block.count_ones() as usize;
            array.as_ref().get_unchecked(small_array_index).assume_init_read()
        }
    }  
    
    /// # Safety
    /// 
    /// * `index` must not be set.
    /// * `mask`'s corresponding bit must be 1.
    #[inline]
    unsafe fn insert_unchecked<Mask: BitBlock>(&mut self, mask: &Mask, index: usize, value: BlockIndices::Item){
        if self.is_big(){
            let array = self.big.1.deref_mut();
            *array.deref_mut().as_mut().get_unchecked_mut(index) = value;
        } else {
            let (mask_u64_populations, array) = &mut self.small;
            let len = *mask_u64_populations.as_ref().last().unwrap_unchecked() as usize + mask.as_array().last().unwrap_unchecked().count_ones() as usize;
            if len == SmallBlockIndices::CAP {
                // TODO: as non-inline function?
                // move to Big
                let mut big: Box<BlockIndices> = Box::new(unsafe{MaybeUninit::zeroed().assume_init()});
                let big_array = big.deref_mut().as_mut(); 
                let mut i = 0;
                mask.traverse_bits(|index|{
                    let value = array.as_ref().get_unchecked(i).assume_init_read();
                    i += 1;
                    
                    *big_array.get_unchecked_mut(index) = value;
                    Continue(()) 
                });
                *big_array.get_unchecked_mut(index) = value;
                *self = Self::from(big);
                return;
            } 
            
            let inner_index = Self::small_array_index(mask_u64_populations, mask, index);
            unsafe{
                let p: *mut _ = array.as_mut().as_mut_ptr().add(inner_index);
                // Shift everything over to make space. (Duplicating the
                // `index`th element into two consecutive places.)
                ptr::copy(p, p.offset(1), len - inner_index);
                // Write it in, overwriting the first copy of the `index`th
                // element.
                ptr::write(p, MaybeUninit::new(value));
            }
            
            for i in (index/64)+1..Mask::size()/64 {
                *mask_u64_populations.as_mut().get_unchecked_mut(i) += 1;
            }
        }
    }    
    
    /// # Safety
    /// 
    /// * `index` must be set.     
    /// * `mask`'s corresponding bit must be 1. (TODO: change)
    #[inline]
    unsafe fn remove_unchecked<Mask: BitBlock>(&mut self, mask: &Mask, index: usize){
        if self.is_big(){
            let array = self.big.1.deref_mut();
            // TODO: go back to small at small/2 size? 
            *array.deref_mut().as_mut().get_unchecked_mut(index) = Primitive::ZERO;
        } else {
            let (mask_u64_populations, array) = &mut self.small;
            let len = *mask_u64_populations.as_ref().last().unwrap_unchecked() as usize + mask.as_array().last().unwrap_unchecked().count_ones() as usize;
            let inner_index = Self::small_array_index(mask_u64_populations, mask, index);
            
            unsafe{
                let len = len - 1;
                let p: *mut _ = array.as_mut().as_mut_ptr().add(inner_index);
                ptr::copy(p.offset(1), p, len - inner_index);
            }
            
            for i in (index/64)+1..Mask::size()/64 {
                *mask_u64_populations.as_mut().get_unchecked_mut(i) -= 1;
            }            
        }
    }    
}

impl<BlockIndices, SmallBlockIndices, MaskU64Populations> Drop for BigSmallArray<BlockIndices, SmallBlockIndices, MaskU64Populations>
where
    BlockIndices: PrimitiveArray,
    SmallBlockIndices: PrimitiveArray<Item=BlockIndices::Item>,
    MaskU64Populations: PrimitiveArray<Item=u8>
{
    #[inline]
    fn drop(&mut self) {
        if self.is_big(){
            unsafe{ ManuallyDrop::drop(&mut self.big.1); }
        }
    }
}

#[derive(Clone)]
pub struct CompactBlock<Mask, MaskU64Populations, BlockIndices, SmallBlockIndices>
where
    BlockIndices: PrimitiveArray,
    SmallBlockIndices: PrimitiveArray<Item=BlockIndices::Item>,
    MaskU64Populations: PrimitiveArray<Item=u8>,
{
    mask: Mask,
    big_small: BigSmallArray<BlockIndices, SmallBlockIndices, MaskU64Populations>
}

impl<Mask, MaskU64Populations, BlockIndices, SmallBlockIndices> Default for CompactBlock<Mask, MaskU64Populations, BlockIndices, SmallBlockIndices>
where
    Mask: BitBlock,
    BlockIndices: PrimitiveArray,
    SmallBlockIndices: PrimitiveArray<Item=BlockIndices::Item>,
    MaskU64Populations: PrimitiveArray<Item=u8>,
{
    #[inline]
    fn default() -> Self {
        Self{
            mask: Mask::zero(),
            big_small:
            BigSmallArray::from(
                (
                /*mask_u64_populations:*/ unsafe{MaybeUninit::zeroed().assume_init()},
                /*array:*/ SmallBlockIndices::UninitArray::uninit_array()
                )
            )
        }
    }
}

impl<Mask, MaskU64Populations, BlockIndices, SmallBlockIndices> CompactBlock<Mask, MaskU64Populations, BlockIndices, SmallBlockIndices>
where
    Mask: BitBlock,
    MaskU64Populations: PrimitiveArray<Item=u8>, 
    BlockIndices: PrimitiveArray,
    SmallBlockIndices: PrimitiveArray<Item=BlockIndices::Item>,
{
    /// # Safety
    ///
    /// index is not checked.
    #[inline]
    pub unsafe fn get_or_insert(
        &mut self,
        index: usize,
        mut f: impl FnMut() -> BlockIndices::Item
    ) -> BlockIndices::Item /*block index*/ {
        let mut block_index = self.big_small.get_or_zero(&self.mask, index);
        if block_index.is_zero(){
            block_index = f();
            unsafe{
                self.big_small.insert_unchecked(&self.mask, index, block_index);
            }
            self.mask.set_bit::<true>(index);
        }
        block_index
    }    
    
    /// # Safety
    ///
    /// * `index` must be set
    /// * `index` is not checked for out-of-bounds.
    #[inline]
    pub unsafe fn remove_unchecked(&mut self, index: usize) {
        // mask
        self.big_small.remove_unchecked(&self.mask, index);
        let prev = self.mask.set_bit::<false>(index);
        debug_assert!(prev);
    }
    
   /* /// # Safety
    ///
    /// - index is not checked for out-of-bounds.
    /// - index is not checked for validity.
    #[inline]
    pub unsafe fn get_unchecked(&self, index: usize) -> BlockIndices::Item {
        self.big_small.get_unchecked(&self.mask, index)
        //self.get(index).unwrap_or(Primitive::ZERO)
    }*/
    
    #[inline]
    pub unsafe fn get_or_zero(&self, index: usize) -> BlockIndices::Item {
        self.big_small.get_or_zero(&self.mask, index)
    }
}

impl<Mask, MaskU64Populations, BlockIndices, SmallBlockIndices> IBlock for CompactBlock<Mask, MaskU64Populations, BlockIndices, SmallBlockIndices>
where
    Mask: BitBlock,
    MaskU64Populations: PrimitiveArray<Item=u8>, 
    BlockIndices: PrimitiveArray,
    SmallBlockIndices: PrimitiveArray<Item=BlockIndices::Item>,
{
    type Mask = Mask;

    #[inline]
    fn mask(&self) -> &Self::Mask {
        &self.mask
    }

    #[inline]
    unsafe fn mask_mut(&mut self) -> &mut Self::Mask {
        &mut self.mask
    }

    type Item = BlockIndices::Item;

    #[inline]
    unsafe fn get_or_zero(&self, index: usize) -> Self::Item {
        self.big_small.get_or_zero(&self.mask, index)
    }

    #[inline]
    unsafe fn get_or_insert(&mut self, index: usize, mut f: impl FnMut() -> Self::Item) -> Self::Item {
        let mut block_index = self.big_small.get_or_zero(&self.mask, index);
        if block_index.is_zero(){
            block_index = f();
            unsafe{
                self.big_small.insert_unchecked(&self.mask, index, block_index);
            }
            self.mask.set_bit::<true>(index);
        }
        block_index
    }

    #[inline]
    unsafe fn remove_unchecked(&mut self, index: usize) {
        // mask
        self.big_small.remove_unchecked(&self.mask, index);
        let prev = self.mask.set_bit::<false>(index);
        debug_assert!(prev);
    }
}
