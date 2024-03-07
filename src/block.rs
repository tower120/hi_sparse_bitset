use std::mem::{MaybeUninit, size_of};
use crate::bit_block::BitBlock;

use crate::{Primitive, PrimitiveArray};
use crate::level::IBlock;

#[derive(Clone)]
pub struct Block<Mask, BlockIndices> {
    mask: Mask,
    /// Next level block indices
    block_indices: BlockIndices,
}

impl<Mask, BlockIndices> Default for Block<Mask, BlockIndices>
where
    Mask: BitBlock,
    BlockIndices: PrimitiveArray
{
    #[inline]
    fn default() -> Self {
        Self {
            mask: Mask::zero(),
            // All indices 0.
            block_indices: unsafe{MaybeUninit::zeroed().assume_init()}
        }
    }
}

impl<Mask, BlockIndices> Block<Mask, BlockIndices>
where
    Mask: BitBlock,
    BlockIndices: PrimitiveArray
{
    #[inline]
    pub unsafe fn from_raw(mask: Mask, block_indices: BlockIndices) -> Self {
        Self{ mask, block_indices }
    }
    
    /// # Safety
    ///
    /// index is not checked.
    #[inline]
    pub unsafe fn get_or_insert(
        &mut self,
        index: usize,
        mut f: impl FnMut() -> BlockIndices::Item
    ) -> BlockIndices::Item {
        // mask
        let exists = self.insert_mask_unchecked(index);

        // indices
        let block_indices = self.block_indices.as_mut();
        if exists {
            *block_indices.get_unchecked(index)
        } else {
            let block_index = f();
            *block_indices.get_unchecked_mut(index) = block_index;
            block_index
        }
    }

    // TODO: remove
    /// Insert mask only
    ///
    /// Return previous mask bit
    ///
    /// # Safety
    ///
    /// - index is not checked for out-of-bounds.
    /// - only safe to call for Block without block_indices (DataBlock)
    #[inline]
    pub unsafe fn insert_mask_unchecked(&mut self, index: usize) -> bool {
        self.mask.set_bit::<true>(index)
    }

    /// Return previous mask bit.
    ///
    /// # Safety
    ///
    /// `index` is not checked for out-of-bounds.
    #[inline]
    pub unsafe fn remove(&mut self, index: usize) -> bool {
        // mask
        let prev = self.mask.set_bit::<false>(index);
        // If we have block_indices section (compile-time check)
        if !size_of::<BlockIndices>().is_zero(){
            let block_indices = self.block_indices.as_mut();
            *block_indices.get_unchecked_mut(index) = Primitive::ZERO;
        }
        prev
    }

    /// # Safety
    ///
    /// index is not checked for out-of-bounds.
    #[inline]
    pub unsafe fn get(&self, index: usize) -> Option<BlockIndices::Item> {
        let exists = self.contains(index);
        if !exists{
            None
        } else {
            Some(self.get_unchecked(index))
        }
    }

    // TODO: remove
    /// # Safety
    ///
    /// - index is not checked for out-of-bounds.
    /// - index is not checked for validity.
    #[inline]
    pub unsafe fn get_unchecked(&self, index: usize) -> BlockIndices::Item {
        let block_indices = self.block_indices.as_ref();
        *block_indices.get_unchecked(index)
    }

    /// # Safety
    ///
    /// index is not checked for out-of-bounds.
    #[inline]
    pub unsafe fn contains(&self, index: usize) -> bool {
        self.mask.get_bit(index)
    }

    #[inline]
    pub fn mask(&self) -> &Mask {
        &self.mask
    }
    
    #[inline]
    pub unsafe fn mask_mut(&mut self) -> &mut Mask{
        &mut self.mask
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        Mask::is_zero(&self.mask)
    }
}

impl<Mask, BlockIndices> IBlock for Block<Mask, BlockIndices>
where
    Mask: BitBlock,
    BlockIndices: PrimitiveArray
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

    /// Same as `get_unchecked`
    #[inline]
    unsafe fn get_or_zero(&self, index: usize) -> Self::Item {
        let block_indices = self.block_indices.as_ref();
        *block_indices.get_unchecked(index)
    }

    #[inline]
    unsafe fn get_or_insert(&mut self, index: usize, mut f: impl FnMut() -> Self::Item) -> Self::Item {
        // mask
        let exists = self.insert_mask_unchecked(index);

        // indices
        let block_indices = self.block_indices.as_mut();
        if exists {
            *block_indices.get_unchecked(index)
        } else {
            let block_index = f();
            *block_indices.get_unchecked_mut(index) = block_index;
            block_index
        }
    }

    #[inline]
    unsafe fn remove_unchecked(&mut self, index: usize) {
        // mask
        self.mask.set_bit::<false>(index);
        // If we have block_indices section (compile-time check)
        if !size_of::<BlockIndices>().is_zero(){
            let block_indices = self.block_indices.as_mut();
            *block_indices.get_unchecked_mut(index) = Primitive::ZERO;
        }
    }
}