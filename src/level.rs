use crate::block::Block;
use crate::{BitBlock, Primitive};

#[derive(Clone)]
pub struct Level<Mask, BlockIndex, BlockIndices>{
    blocks: Vec<Block<Mask, BlockIndex, BlockIndices>>,
    
    /// Single linked list of empty block indices.
    /// Mask of empty block used as a "next free block".
    /// u64::MAX - terminator.
    root_empty_block: u64,
}

impl<Mask, BlockIndex, BlockIndices> Level<Mask, BlockIndex, BlockIndices>
where
    Mask: BitBlock,
    BlockIndex: Primitive,
    BlockIndices: AsRef<[BlockIndex]> + AsMut<[BlockIndex]> + Clone
{
    #[inline]
    pub fn new(blocks: Vec<Block<Mask, BlockIndex, BlockIndices>>) -> Self{
        Self{
            blocks,
            root_empty_block: u64::MAX,
        }
    }
    
    // TODO: remove, have get_unchecked() instead?
    #[inline]
    pub fn blocks(&self) -> &[Block<Mask, BlockIndex, BlockIndices>]{
        self.blocks.as_slice()
    }

    // TODO: remove?
    #[inline]
    pub fn blocks_mut(&mut self) -> &mut [Block<Mask, BlockIndex, BlockIndices>]{
        self.blocks.as_mut_slice()
    }

    /// Next empty block link
    /// 
    /// Block's mask used as index to next empty block
    #[inline]
    unsafe fn next_empty_block_index(
        block: &mut Block<Mask, BlockIndex, BlockIndices>
    ) -> &mut u64 {
        block.mask.as_array_mut().get_unchecked_mut(0)
    }
    
    #[inline]
    fn pop_empty_block(&mut self) -> Option<usize> {
        if self.root_empty_block == u64::MAX {
            return None;
        }
            
        let index = self.root_empty_block as usize;
        unsafe{
            let empty_block = self.blocks.get_unchecked_mut(index);
            let next_empty_block_index = Self::next_empty_block_index(empty_block); 
            
            // update list root 
            self.root_empty_block = *next_empty_block_index;
            
            // restore original mask zero state
            *next_empty_block_index = 0;
        }
        Some(index)
    }

    /// # Safety
    /// 
    /// block must be empty and not in use!
    #[inline]
    unsafe fn push_empty_block(&mut self, block_index: usize){
        let empty_block = self.blocks.get_unchecked_mut(block_index);
        let next_empty_block_index = Self::next_empty_block_index(empty_block);
        *next_empty_block_index = self.root_empty_block;
        
        self.root_empty_block = block_index as u64;
    }

    #[inline]
    pub fn insert_empty_block(&mut self) -> usize {
        if let Some(index) = self.pop_empty_block(){
            index
        } else {
            let index = self.blocks.len();
            self.blocks.push(Block::empty());
            index
        }
    }
    
    #[inline]
    pub fn insert_block(
        &mut self, 
        block: Block<Mask, BlockIndex, BlockIndices>
    ) -> usize {
        if let Some(index) = self.pop_empty_block(){
            unsafe{
                *self.blocks.get_unchecked_mut(index) = block;
            }
            index
        } else {
            let index = self.blocks.len();
            self.blocks.push(block);
            index
        }
    }

    /// # Safety
    ///
    /// block_index and block emptiness are not checked.
    #[inline]
    pub unsafe fn remove_empty_block_unchecked(&mut self, block_index: usize) {
        self.push_empty_block(block_index);
        // Do not touch block itself - it should be already empty
    }
}