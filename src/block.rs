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
        let exists = self.mask.set_bit::<true>(index);

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