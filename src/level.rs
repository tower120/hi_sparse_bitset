use crate::BitBlock;
use crate::primitive::Primitive;

pub trait IBlock: Sized + Default{
    type Mask: BitBlock;
    
    fn mask(&self) -> &Self::Mask; 
    unsafe fn mask_mut(&mut self) -> &mut Self::Mask;
    
    // TODO: BlockIndex
    type Item: Primitive;
    
    /*/// # Safety
    ///
    /// - index is not checked for out-of-bounds.
    /// - index is not checked for validity (must exist).
    unsafe fn get_unchecked(&self, index: usize) -> Self::Item;*/
    
    /// Returns 0 if item does not exist at `index`.
    /// 
    /// # Safety
    /// 
    /// index is not checked for out-of-bounds.
    unsafe fn get_or_zero(&self, index: usize) -> Self::Item;
    
    /// # Safety
    ///
    /// index is not checked.
    unsafe fn get_or_insert(
        &mut self,
        index: usize,
        f: impl FnMut() -> Self::Item
    ) -> Self::Item;
    
    /// Return previous mask bit.
    /// 
    /// # Safety
    ///
    /// * `index` must be set
    /// * `index` is not checked for out-of-bounds.
    unsafe fn remove_unchecked(&mut self, index: usize);
    
    #[inline]
    fn is_empty(&self) -> bool {
        Self::Mask::is_zero(self.mask())
    }
}

#[derive(Clone)]
pub struct Level<Block: IBlock>{
    blocks: Vec<Block>,
    
    /// Single linked list of empty block indices.
    /// Mask of empty block used as a "next free block".
    /// u64::MAX - terminator.
    root_empty_block: u64,
}

impl<Block: IBlock> Default for Level<Block> {
    #[inline]
    fn default() -> Self {
        Self{
            //Always have empty block at index 0.
            blocks:vec![Default::default()],
            root_empty_block: u64::MAX,
        }
    }
}

impl<Block: IBlock> Level<Block> {
    #[inline]
    pub fn blocks(&self) -> &[Block] {
        self.blocks.as_slice()
    }

    #[inline]
    pub fn blocks_mut(&mut self) -> &mut [Block] {
        self.blocks.as_mut_slice()
    }

    /// Next empty block link
    /// 
    /// Block's mask used as index to next empty block
    #[inline]
    unsafe fn next_empty_block_index(block: &mut Block) -> &mut u64 {
        block.mask_mut().as_array_mut().get_unchecked_mut(0)
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