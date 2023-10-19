use num_traits::{AsPrimitive, PrimInt};

#[derive(Default, Clone)]
pub struct Level<LevelBlock, LevelBlockIndex>{
    blocks: Vec<LevelBlock>,
    free_block_indices: Vec<LevelBlockIndex>,
}
impl<LevelBlock: Default, LevelBlockIndex: PrimInt + 'static> Level<LevelBlock, LevelBlockIndex> {
    #[inline]
    pub fn blocks(&self) -> &[LevelBlock]{
        self.blocks.as_slice()
    }

    #[inline]
    pub fn blocks_mut(&mut self) -> &mut [LevelBlock]{
        self.blocks.as_mut_slice()
    }

    #[inline]
    pub fn insert_block(&mut self) -> LevelBlockIndex {
        if let Some(index) = self.free_block_indices.pop(){
            index
        } else {
            let index = self.blocks.len();
            self.blocks.push(Default::default());
            unsafe {
                // index as LevelBlockIndex
                LevelBlockIndex::from(index).unwrap_unchecked()
            }
        }
    }

    /// # Safety
    ///
    /// block_index and block emptiness unchecked.
    #[inline]
    pub unsafe fn remove_empty_block_unchecked(&mut self, block_index: LevelBlockIndex) {
        self.free_block_indices.push(block_index);
        // Do not touch block itself - it should be already empty
    }
}