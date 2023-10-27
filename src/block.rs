use std::marker::PhantomData;
use std::mem::{MaybeUninit, size_of};
use crate::bit_block::BitBlock;

use num_traits::{PrimInt, Zero};
use crate::INTERSECTION_ONLY;

#[derive(Clone)]
pub struct Block<Mask, BlockIndex, BlockIndices>
where
    Mask: BitBlock,
    BlockIndices: AsRef<[BlockIndex]> + AsMut<[BlockIndex]> + Clone
{
    mask: Mask,
    /// Next level block indices
    block_indices: BlockIndices,
    phantom: PhantomData<BlockIndex>
}

impl<Mask, BlockIndex, BlockIndices> Default for Block<Mask, BlockIndex, BlockIndices>
where
    Mask: BitBlock,
    BlockIndices: AsRef<[BlockIndex]> + AsMut<[BlockIndex]> + Clone
{
    #[inline]
    fn default() -> Self {
        Self {
            mask: Mask::zero(),
            block_indices:
                if INTERSECTION_ONLY{
                    unsafe{MaybeUninit::uninit().assume_init()}
                } else {
                    // All indices 0.
                    unsafe{MaybeUninit::zeroed().assume_init()}
                },
            phantom: PhantomData
        }
    }
}

impl<Mask, BlockIndex, BlockIndices> Block<Mask, BlockIndex, BlockIndices>
where
    Mask: BitBlock,
    BlockIndex: PrimInt,
    BlockIndices: AsRef<[BlockIndex]> + AsMut<[BlockIndex]> + Clone
{
    /// # Safety
    ///
    /// index is not checked.
    #[inline]
    pub unsafe fn get_or_insert(
        &mut self,
        index: usize,
        mut f: impl FnMut() -> BlockIndex
    ) -> BlockIndex {
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

    /// Return previous mask bit
    ///
    /// # Safety
    ///
    /// index is not checked for out-of-bounds.
    #[inline]
    pub unsafe fn remove(&mut self, index: usize) -> bool {
        // mask
        let prev = self.mask.set_bit::<false>(index);
        // don't touch block_index - it state is irrelevant
        if !INTERSECTION_ONLY {
            // If we have Blocks section (compile-time check)
            if !size_of::<BlockIndices>().is_zero(){
                let block_indices = self.block_indices.as_mut();
                *block_indices.get_unchecked_mut(index) = BlockIndex::zero();
            }
        }
        prev
    }

    /// # Safety
    ///
    /// index is not checked for out-of-bounds.
    #[inline]
    pub unsafe fn get(&self, index: usize) -> Option<BlockIndex> {
        let exists = self.contains(index);
        if !exists{
            None
        } else {
            Some(self.get_unchecked(index))
        }
    }

    /// # Safety
    ///
    /// - index is not checked for out-of-bounds.
    /// - index is not checked for validity.
    #[inline]
    pub unsafe fn get_unchecked(&self, index: usize) -> BlockIndex {
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
    pub fn is_empty(&self) -> bool {
        Mask::is_zero(&self.mask)
    }
}
