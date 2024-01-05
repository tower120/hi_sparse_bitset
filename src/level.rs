use crate::block::Block;
use crate::{BitBlock, INTERSECTION_ONLY, Primitive};

#[derive(Clone)]
pub struct Level<Mask, BlockIndex, BlockIndices>{
    blocks: Vec<Block<Mask, BlockIndex, BlockIndices>>,
    
    /// Single linked list of empty block indices.
    /// Mask of empty block used as a "next free block".
    /// u64::MAX - terminator.
    root_empty_block: u64,
}

impl<Mask, BlockIndex, BlockIndices> Default for Level<Mask, BlockIndex, BlockIndices>
where
    Mask: BitBlock,
    BlockIndices: AsRef<[BlockIndex]> + AsMut<[BlockIndex]> + Clone
{
    #[inline]
    fn default() -> Self {
        Self{
            blocks: if !INTERSECTION_ONLY{
                //Always have empty block at index 0.
                vec![Default::default()]
            } else {
                Default::default()
            },
            root_empty_block: u64::MAX,
        }
    }
}

impl<Mask, BlockIndex, BlockIndices> Level<Mask, BlockIndex, BlockIndices>
where
    Mask: BitBlock,
    BlockIndex: Primitive,
    BlockIndices: AsRef<[BlockIndex]> + AsMut<[BlockIndex]> + Clone
{
    #[inline]
    pub fn blocks(&self) -> &[Block<Mask, BlockIndex, BlockIndices>]{
        self.blocks.as_slice()
    }

    #[inline]
    pub fn blocks_mut(&mut self) -> &mut [Block<Mask, BlockIndex, BlockIndices>]{
        self.blocks.as_mut_slice()
    }
    
    #[inline]
    fn pop_empty_block(&mut self) -> Option<usize> {
        if self.root_empty_block == u64::MAX {
            return None;
        }
            
        let index = self.root_empty_block as usize;
        unsafe{
            let empty_block = self.blocks.get_unchecked_mut(index);
            let next_empty_block = empty_block.raw_mask_mut().first_u64_mut(); 
            
            // update list root 
            self.root_empty_block = *next_empty_block;
            
            // restore original mask zero state
            *next_empty_block = 0;
        }
        Some(index)
    }

    /// # Safety
    /// 
    /// block must be empty and not in use!
    #[inline]
    unsafe fn push_empty_block(&mut self, block_index: usize){
        let empty_block = self.blocks.get_unchecked_mut(block_index);
        let next_empty_block = empty_block.raw_mask_mut().first_u64_mut();
        *next_empty_block = self.root_empty_block;
        
        self.root_empty_block = block_index as u64;
    }

    #[inline]
    pub fn insert_block(&mut self) -> usize {
        if let Some(index) = self.pop_empty_block(){
            index
        } else {
            let index = self.blocks.len();
            self.blocks.push(Default::default());
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