use std::mem::{MaybeUninit, size_of};
use std::ops::{Deref, DerefMut};
use std::ops::ControlFlow::Continue;
use std::ptr;
use crate::bit_utils::get_bit_unchecked;
use crate::{BitBlock, PREALLOCATED_EMPTY_BLOCK};
use crate::level::IBlock;
use crate::primitive::Primitive;
use crate::PrimitiveArray;

// TODO: could be smaller?
type MaskU64Populations = [u8; 2];

#[derive(Clone)]
enum BigSmallArray<BlockIndices, SmallBlockIndices>{
    Big(Box<BlockIndices>),
    Small{
        /// mask's bit-population at the start of each u64
        mask_u64_populations: MaskU64Populations,
        array: SmallBlockIndices,
        // TODO: This can be deduced from mask_u64_populations
        array_len: u8,
    }
}
impl<BlockIndices, SmallBlockIndices> BigSmallArray<BlockIndices, SmallBlockIndices>
where
    BlockIndices: PrimitiveArray,
    SmallBlockIndices: PrimitiveArray<Item=BlockIndices::Item>,
{
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
        let offset = *mask_u64_populations.get_unchecked(u64_index);
        offset as usize + block.count_ones() as usize
    }
    
    /// # Safety
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
    }
    
    #[inline]
    unsafe fn get_or_zero<Mask: BitBlock>(&self, mask: &Mask, index: usize) -> BlockIndices::Item {
        match self{
            BigSmallArray::Big(array) => {
                unsafe { *array.deref().as_ref().get_unchecked(index) }
            }
            BigSmallArray::Small { mask_u64_populations, array, array_len } => {
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
                
                let offset = *mask_u64_populations.get_unchecked(u64_index);
                let small_array_index = offset as usize + block.count_ones() as usize;
                *array.as_ref().get_unchecked(small_array_index)
            }
        }
    }    
    
    /// # Safety
    /// 
    /// * `index` must not be set.
    #[inline]
    unsafe fn insert_unchecked<Mask: BitBlock>(&mut self, mask: &Mask, index: usize, value: BlockIndices::Item){
        match self{
            BigSmallArray::Big(array) => {
                unsafe {
                    *array.deref_mut().as_mut().get_unchecked_mut(index) = value;
                }
            }
            BigSmallArray::Small { mask_u64_populations, array, array_len } => {
                let len = *array_len as usize; 
                if len == SmallBlockIndices::CAP {
                    // TODO: as non-inline function?
                    // move to Big
                    let mut big: Box<BlockIndices> = Box::new(unsafe{MaybeUninit::zeroed().assume_init()});
                    let big_array = big.deref_mut().as_mut(); 
                    let mut i = 0;
                    mask.traverse_bits(|index|{
                        let value = *array.as_ref().get_unchecked(i);
                        i += 1;
                        
                        *big_array.get_unchecked_mut(index) = value;
                        Continue(()) 
                    });
                    *big_array.get_unchecked_mut(index) = value;
                    *self = BigSmallArray::Big(big);
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
                    ptr::write(p, value);
                }
                *array_len += 1;
                
                for i in (index/64)+1..Mask::size()/64 {
                    *mask_u64_populations.get_unchecked_mut(i) += 1;
                }
            }
        }
    }
    
    /// # Safety
    /// 
    /// `index` must be set.     
    #[inline]
    unsafe fn remove_unchecked<Mask: BitBlock>(&mut self, mask: &Mask, index: usize){
        match self{
            BigSmallArray::Big(array) => {
                // TODO: go back to small at small/2 size? 
                *array.deref_mut().as_mut().get_unchecked_mut(index) = Primitive::ZERO;
            }
            BigSmallArray::Small { mask_u64_populations, array, array_len } => {
                let inner_index = Self::small_array_index(mask_u64_populations, mask, index);
                
                *array_len -= 1;
                unsafe{
                    let len = *array_len as usize; 
                    let p: *mut _ = array.as_mut().as_mut_ptr().add(inner_index);
                    ptr::copy(p.offset(1), p, len - inner_index);
                }
                
                for i in (index/64)+1..Mask::size()/64 {
                    *mask_u64_populations.get_unchecked_mut(i) -= 1;
                }                
            }
        }
    }
}

#[derive(Clone)]
pub struct CompactBlock<Mask, BlockIndices, SmallBlockIndices>{
    mask: Mask,
    big_small: BigSmallArray<BlockIndices, SmallBlockIndices>
}

impl<Mask, BlockIndices, SmallBlockIndices> Default for CompactBlock<Mask, BlockIndices, SmallBlockIndices>
where
    Mask: BitBlock,
{
    #[inline]
    fn default() -> Self {
        Self{
            mask: Mask::zero(),
            big_small:
            BigSmallArray::Small {
                mask_u64_populations: Default::default(),
                array: unsafe{MaybeUninit::uninit().assume_init()},
                array_len: 0,
            }
        }
    }
}

impl<Mask, BlockIndices, SmallBlockIndices> CompactBlock<Mask, BlockIndices, SmallBlockIndices>
where
    Mask: BitBlock,
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

impl<Mask, BlockIndices, SmallBlockIndices> IBlock for CompactBlock<Mask, BlockIndices, SmallBlockIndices>
where
    Mask: BitBlock
{
    type Mask = Mask;

    #[inline]
    fn mask(&self) -> &Self::Mask {
        &self.mask
    }

    #[inline]
    fn mask_mut(&mut self) -> &mut Self::Mask {
        &mut self.mask
    }
}
