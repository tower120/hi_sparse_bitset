use std::marker::PhantomData;
use std::mem::{MaybeUninit, size_of};
use crate::bit_block::{BitBlock, BitBlockFull};

use crate::Primitive;

#[derive(Clone)]
pub struct Block<Mask, BlockIndex, BlockIndices> {
    pub mask: Mask,
    /// Next level block indices
    pub block_indices: BlockIndices,
    
    // TODO: change somehow
    pub full_mask: Mask,
    
    phantom: PhantomData<BlockIndex>
}

impl<Mask, BlockIndex, BlockIndices> Block<Mask, BlockIndex, BlockIndices>
where
    Mask: BitBlock,
    BlockIndex: Primitive,
    BlockIndices: AsRef<[BlockIndex]> + AsMut<[BlockIndex]> + Clone
{
    #[inline]
    pub fn empty() -> Self{
        Self {
            mask: Mask::zero(),
            full_mask: Mask::zero(),
            // All indices 0.
            block_indices: unsafe{ MaybeUninit::zeroed().assume_init() },
            phantom: PhantomData
        }
    }
    
    #[inline]
    pub fn full() -> Self
    where
        Mask: BitBlockFull
    {
        // All indices 1.
        let block_indices = unsafe {
            let mut u = MaybeUninit::<BlockIndices>::uninit();
            u.assume_init_mut().as_mut().fill(BlockIndex::from_usize(1));
            u.assume_init()
        };
        
        Self {
            mask: Mask::full(),
            full_mask: Mask::full(),
            block_indices,
            phantom: PhantomData
        }
    }
    
    #[inline]
    pub const fn size() -> usize {
        1 << Mask::SIZE_POT_EXPONENT
    }
    
    /// # Safety
    ///
    /// index is not checked.
    #[inline]
    pub unsafe fn get_or_insert(
        &mut self,
        index: usize,
        f: impl FnOnce() -> BlockIndex
    ) -> BlockIndex {
        // mask
        self.mask.set_bit::<true>(index);

        // indices
        let block_indices = self.block_indices.as_mut();
        let index_ref = block_indices.get_unchecked_mut(index);
        let index = *index_ref;
        if index.is_zero(){
            let block_index = f();
            *index_ref = block_index;
            block_index
        } else {
            index
        }
    }

    /// Return (previous mask bit, block hint) 
    ///
    /// # Safety
    ///
    /// index is not checked for out-of-bounds.
    #[inline]
    pub unsafe fn remove(&mut self, index: usize) -> (bool, u64) {
        // If we have block_indices section (compile-time check),
        // point to empty block (0).
        if !size_of::<BlockIndices>().is_zero(){
            let block_indices = self.block_indices.as_mut();
            *block_indices.get_unchecked_mut(index) = BlockIndex::ZERO;
        }
        
        // mask
        self.mask.set_bit::<false>(index)
    }

    // TODO: unused?
    /// # Safety
    ///
    /// index is not checked for out-of-bounds.
    #[inline]
    pub unsafe fn contains_unchecked(&self, index: usize) -> bool {
        self.mask.get_bit(index)
    }

    // TODO: remove this?
    #[inline]
    pub fn is_empty(&self) -> bool {
        Mask::is_zero(&self.mask)
    }
    
    #[inline]
    pub fn is_full(&self) -> bool
    where
        Mask: BitBlockFull
    {
        Mask::is_full(&self.mask)
    }
}
