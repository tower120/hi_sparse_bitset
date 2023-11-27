use std::mem::{ManuallyDrop, MaybeUninit};

use crate::bit_block::BitBlock;
use crate::bit_queue::BitQueue;
use crate::bitset_interface::{BitSetBase, LevelMasksExt};
use crate::level_indices;

use super::*;

/// Caching iterator.
///
/// Cache pre-data level block pointers, making data blocks access faster.
/// Also, can discard (on pre-data level) sets with empty level1 blocks from iteration.
/// (See [binary_op] - this have no effect for AND operation, but can speed up all other)
///
/// # Memory footprint
///
/// This iterator may store some data in its internal state.
/// Amount of memory used by cache depends on [cache] type.
/// Cache affects only [reduce] operations.
/// 
/// [cache]: crate::cache
/// [reduce]: crate::reduce()
/// [binary_op]: crate::binary_op
pub struct CachingBlockIter<T>
where
    T: LevelMasksExt,
{
    virtual_set: T,
    state: State<T::Config>,
    cache_data: ManuallyDrop<T::CacheData>,
    /// Never drop - since we're guaranteed to have them POD.
    level1_blocks: MaybeUninit<T::Level1Blocks>,
}

impl<T> CachingBlockIter<T>
where
    T: LevelMasksExt,
{
    pub fn move_to(&mut self, cursor: BlockIterCursor){
        //let mut cache_data = virtual_set.make_cache();
        //let mut level1_blocks = MaybeUninit::uninit();
        
        // Here we update State.
        
        // Reset level0 mask if we not in "initial state"
        if self.state.level0_index != usize::MAX{
            self.state.level0_iter = self.virtual_set.level0_mask().bits_iter();    
        }
        
        // Mask out level0 mask
        self.state.level0_iter.zero_first_n(cursor.level0_index);

        if let Some(level0_index) = self.state.level0_iter.next(){
            self.state.level0_index = level0_index;
            
            // generate level1 mask, and update cache.
            let (level1_mask, valid) = unsafe {
                self.virtual_set.update_level1_blocks(&mut self.cache_data, &mut self.level1_blocks, level0_index)
            };
            if !valid {
                // level1_mask can not be empty here
                unsafe { std::hint::unreachable_unchecked() }
            }
            self.state.level1_iter = level1_mask.bits_iter();
            
            // TODO: can we mask SIMD block directly? 
            // mask out level1 mask, if this is block pointed by cursor
            if level0_index == cursor.level0_index{
                self.state.level1_iter.zero_first_n(cursor.level1_next_index);
            }
        } else {
            // absolutely empty
            self.state.level1_iter  = BitQueue::empty();
            self.state.level0_index = /*TODO: MAX LEVEL INDEX*/0;
        }
    }
    

/*     pub fn resume_impl(&mut self,  cursor: BlockIterCursor) {
        let level0_iter = &mut self.state.level0_iter;
        let virtual_set = &mut self.virtual_set;
        let mut cache_data = &mut self.cache_data;
        let mut level1_blocks = &mut self.level1_blocks;
            
        level0_iter.zero_first_n(cursor.level0_index);
        
        let (level0_index, level1_iter) = 
        if let Some(level0_index) = level0_iter.next(){
            // TODO: This can be skipped, if level1 indices equals
            // generate level1 mask, and update cache.
            let (level1_mask, valid) = unsafe {
                virtual_set.update_level1_blocks(&mut cache_data, &mut level1_blocks, level0_index)
            };
            if !valid {
                // level1_mask can not be empty here
                unsafe { std::hint::unreachable_unchecked() }
            }
            let mut level1_iter = level1_mask.bits_iter();
            
            // TODO: we can mask SIMD block directly? 
            // mask out, if this is block pointed by cursor
            if level0_index == cursor.level0_index{
                level1_iter.zero_first_n(cursor.level1_next_index);
            }
            
            (level0_index, level1_iter)
        } else {
            // absolutely empty
            (/*TODO: MAX LEVEL INDEX*/0,  BitQueue::empty())
        };
        
        self.state.level1_iter = level1_iter;
        self.state.level0_index = level0_index;
    }
    
    pub fn resume(virtual_set: T, cursor: BlockIterCursor) -> Self {
        let mut cache_data = virtual_set.make_cache();
        let mut level1_blocks = MaybeUninit::uninit();
        
        let mut level0_iter = virtual_set.level0_mask().bits_iter();
        level0_iter.zero_first_n(cursor.level0_index);

        let (level0_index, level1_iter) = 
        if let Some(level0_index) = level0_iter.next(){
            // generate level1 mask, and update cache.
            let (level1_mask, valid) = unsafe {
                virtual_set.update_level1_blocks(&mut cache_data, &mut level1_blocks, level0_index)
            };
            if !valid {
                // level1_mask can not be empty here
                unsafe { std::hint::unreachable_unchecked() }
            }
            let mut level1_iter = level1_mask.bits_iter();
            
            // TODO: can we mask SIMD block directly? 
            // mask out, if this is block pointed by cursor
            if level0_index == cursor.level0_index{
                level1_iter.zero_first_n(cursor.level1_next_index);
            }
            
            (level0_index, level1_iter)
        } else {
            // absolutely empty
            (/*TODO: MAX LEVEL INDEX*/0,  BitQueue::empty())
        };
        
        let state = State{
            level0_iter,
            level1_iter,
            level0_index,
        };        
        
        Self{
            virtual_set,
            state,
            cache_data: ManuallyDrop::new(cache_data),
            level1_blocks
        }
    } */
}


impl<T> BlockIterator for CachingBlockIter<T>
where
    T: LevelMasksExt,
{
    type BitSet = T;

    #[inline]
    fn new(virtual_set: T) -> Self {
        let level0_iter = virtual_set.level0_mask().bits_iter();
        //let level0_index = level0_iter.current();
        let state = State{
            level0_iter,
            level1_iter: BitQueue::empty(),
            level0_index: usize::MAX,    // Marker, that we in "intial state"
        };
        let cache_data = virtual_set.make_cache();
        Self{
            virtual_set,
            state,
            cache_data: ManuallyDrop::new(cache_data),
            level1_blocks: MaybeUninit::uninit()
        }
    }

    #[inline]
    fn cursor(&self) -> BlockIterCursor {
        if self.state.level0_index == usize::MAX{
            return BlockIterCursor::default();
        }
        
        BlockIterCursor {
            level0_index     : self.state.level0_index,
            level1_next_index: self.state.level1_iter.current(),
        }
    }

    type IndexIter = CachingIndexIter<T>;

    #[inline]
    fn as_indices(self) -> CachingIndexIter<T> {
        CachingIndexIter::new(self)
    }
    
    fn skip_to(&mut self, cursor: BlockIterCursor)
    {
        self.move_to(cursor)
        /*// TODO: optimize
        
        // 1. check if cursor NOT behind the iter
        if self.state.level0_index > cursor.level0_index
        {
            return;
        }
        let t = self.state.level1_iter.current();
        
        if self.state.level1_iter.current() == (1 << <T::Config as IConfig>::Level1BitBlock::SIZE_POT_EXPONENT){
            // level1_iter was never inited
        } else if self.state.level1_iter.current() > cursor.level1_next_index{
            return;
        }
        
        self.resume_impl(cursor);*/
    }
    
/*    #[inline]
    fn skip_to(&mut self, cursor: BlockIterCursor) {
        // level 0
        let original_level0_index = self.state.level0_iter.current();
        self.state.level0_iter.zero_first_n(cursor.level0_index);
        let new_level0_index = self.state.level0_iter.current();
        
        if new_level0_index == 0{
            // nothing to do
            return;
        }
        
        // level 1
        if new_level0_index == cursor.level0_index{
            // TODO: conditionally skip?
            // load/compute data
            {
                let index = new_level0_index - 1;
                let (level1_mask, valid) = unsafe {
                    self.virtual_set.update_level1_blocks(&mut self.cache_data, &mut self.level1_blocks, index)
                };
                if !valid {
                    // level1_mask can not be empty here
                    unsafe { std::hint::unreachable_unchecked() }
                }
                self.state.level1_iter = level1_mask.bits_iter();            
            }
            
            // mask out
            self.state.level1_iter.zero_first_n(cursor.level1_index);
            
            self.state.level0_index = new_level0_index - 1;
        } else if /*we jumped forward?*/ new_level0_index > original_level0_index{
            //self.state.level0_index = new_level0_index - 1;
            self.state.level1_iter = BitQueue::empty();
        } else /*cursor behind current iterator state*/ {
            // leave as is
        }   
    }    */

    /*#[inline]
    fn skip_to(&mut self, cursor: BlockIterCursor) {
        // get level0_index from level0_iter since we did not set it 
        // during iterator construction
        let level0_index = self.state.level0_iter.current();

        use std::cmp::Ordering;
        match Ord::cmp(&cursor.level0_index, &level0_index){
            Ordering::Less => {
                // we're ahead of cursor
                return;
            }
            Ordering::Equal => {
                // We never inited level1_iter before?
                if self.state.level1_iter.is_empty(){
                    // actually consume level0 bit
                    {                        
                        let index = self.state.level0_iter.next();
                        if index.is_none(){
                            // bitset completely empty - nothing to do
                            return;
                        }
                        debug_assert_eq!(index.unwrap(), level0_index);
                    }

                    let (level1_mask, valid) = unsafe {
                        self.virtual_set.update_level1_blocks(&mut self.cache_data, &mut self.level1_blocks, level0_index)
                    };
                    if !valid {
                        // level1_mask can not be empty here
                        unsafe { std::hint::unreachable_unchecked() }
                    }

                    self.state.level1_iter = level1_mask.bits_iter();
                }

                // mask out level1
                /*unsafe {
                    self.state.level1_iter.zero_first_n_unchecked(cursor.level1_index);
                }*/
                self.state.level1_iter.zero_first_n(cursor.level1_index);
                
                // Do not update cache, since we did not switch level0_iter
                
                /*// TODO: UPDATE CACHE!! ??
                {
                    let (level1_mask, valid) = unsafe {
                        self.virtual_set.update_level1_blocks(&mut self.cache_data, &mut self.level1_blocks, level0_index)
                    };
                    if !valid {
                        // level1_mask can not be empty here
                        unsafe { std::hint::unreachable_unchecked() }
                    }                    
                }*/
            }
            Ordering::Greater => {
                // we're before cursor
                /*unsafe {
                    self.state.level0_iter.zero_first_n_unchecked(cursor.level0_index);
                }*/
                self.state.level0_iter.zero_first_n(cursor.level0_index);
                self.state.level0_index = cursor.level0_index;
                self.state.level1_iter  = BitQueue::empty();
            }
        }
    }*/    
}

impl<T> Iterator for CachingBlockIter<T>
where
    T: LevelMasksExt,
{
    type Item = DataBlock<<T::Config as IConfig>::DataBitBlock>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self { virtual_set, state, cache_data, level1_blocks } = self;

        let level1_index =
            loop {
                if let Some(index) = state.level1_iter.next() {
                    break index;
                } else {
                    //update level0
                    if let Some(index) = state.level0_iter.next() {
                        state.level0_index = index;

                        let (level1_mask, valid) = unsafe {
                            virtual_set.update_level1_blocks(cache_data, level1_blocks, index)
                        };
                        if !valid {
                            // level1_mask can not be empty here
                            unsafe { std::hint::unreachable_unchecked() }
                        }
                        state.level1_iter = level1_mask.bits_iter();
                    } else {
                        return None;
                    }
                }
            };

        let data_intersection = unsafe {
            T::data_mask_from_blocks(level1_blocks.assume_init_ref(), level1_index)
        };

        let block_start_index =
            data_block_start_index::<<T as BitSetBase>::Config>(
                state.level0_index, level1_index,
            );

        Some(DataBlock { start_index: block_start_index, bit_block: data_intersection })
    }
}

impl<T> Drop for CachingBlockIter<T>
where
    T: LevelMasksExt
{
    #[inline]
    fn drop(&mut self) {
        self.virtual_set.drop_cache(&mut self.cache_data);
    }
}

//pub type CachingIndexIter<T> = IndexIter<CachingBlockIter<T>>;
pub type CachingIndexIter<T> = CachingIndexIter2<T>;




pub struct CachingIndexIter2<T>
where
    T: LevelMasksExt,
{
    block_iter: CachingBlockIter<T>,
    data_block_iter: DataBlockIter<<T::Config as IConfig>::DataBitBlock>,
}

impl<T> CachingIndexIter2<T>
where
    T: LevelMasksExt,
{
    #[inline]
    pub fn new(block_iter: CachingBlockIter<T>) -> Self{
        Self{
            block_iter,
            data_block_iter: DataBlockIter::empty()
        }
    }
    
    pub fn move_to(&mut self, cursor: IndexIterCursor){
        self.block_iter.move_to(cursor.block_cursor.clone()/*TODO: Make block_cursor Copy*/);
        
        self.data_block_iter = 
        if let Some(data_block) = self.block_iter.next(){
            let mut data_block_iter = data_block.into_iter();
            
            // mask out, if this is block pointed by cursor
            let cursor_block_start_index = data_block_start_index::<T::Config>(
                cursor.block_cursor.level0_index, 
                cursor.block_cursor.level1_next_index /*this is current index*/,
            );
            if data_block_iter.start_index == cursor_block_start_index{
                data_block_iter.bit_block_iter.zero_first_n(cursor.data_next_index);
            }
            
            data_block_iter
        } else {
            // absolutely empty
            DataBlockIter::empty()
        };        
    }
    
   /*  #[inline]
    fn resume_impl(&mut self, cursor: IndexIterCursor) {
        let cursor_block_start_index = data_block_start_index::<T::Config>(
            cursor.block_cursor.level0_index, 
            cursor.block_cursor.level1_next_index /*this is current index*/,
        );
        
        self.block_iter.resume_impl(cursor.block_cursor);
        
        let block_iter = &mut self.block_iter;
        
        self.data_block_iter = 
            if let Some(data_block) = block_iter.next(){
                let mut data_block_iter = data_block.into_iter();
                
                // mask out, if this is block pointed by cursor
                if data_block_iter.start_index == cursor_block_start_index{
                    data_block_iter.bit_block_iter.zero_first_n(cursor.data_next_index);
                }
                
                data_block_iter
            } else {
                // absolutely empty
                DataBlockIter::empty()
            };
    } */
    
    
/*     #[inline]
    pub fn resume(virtual_set: T, cursor: IndexIterCursor) -> Self{
        let cursor_block_start_index = data_block_start_index::<T::Config>(
            cursor.block_cursor.level0_index, 
            cursor.block_cursor.level1_next_index /*this is current index*/,
        );
        
        let mut block_iter = CachingBlockIter::resume(virtual_set, cursor.block_cursor);
        
        let data_block_iter = 
        if let Some(data_block) = block_iter.next(){
            let mut data_block_iter = data_block.into_iter();
            
            // mask out, if this is block pointed by cursor
            if data_block_iter.start_index == cursor_block_start_index{
                data_block_iter.bit_block_iter.zero_first_n(cursor.data_next_index);
            }
            
            data_block_iter
        } else {
            // absolutely empty
            DataBlockIter::empty()
        };

        Self{
            block_iter,
            data_block_iter
        }
    } */
    
}

impl<T> IndexIterator for CachingIndexIter2<T>
where
    T: LevelMasksExt,
{
    type BlockIter = CachingBlockIter<T>;

    #[inline]
    fn as_blocks(self) -> Self::BlockIter{
        self.block_iter
    }
    
    fn skip_to(&mut self, cursor: IndexIterCursor) {
        self.move_to(cursor)
        /*let cursor_block_start_index = data_block_start_index::<T::Config>(
            cursor.block_cursor.level0_index, 
            cursor.block_cursor.level1_next_index /*this is current index*/,
        );
        
        if self.data_block_iter.start_index > cursor_block_start_index{
            return;
        }
        
        self.resume_impl(cursor);*/
    }

/*    #[inline]
    fn skip_to(&mut self, cursor: IndexIterCursor) {
        let cursor_level0_index = cursor.block_cursor.level0_index;
        let cursor_level1_index = cursor.block_cursor.level1_index;
        self.block_iter.skip_to(cursor.block_cursor);

        // Update Data Level iterator only if we at the cursor level blocks. 
        // TODO: we already did this check in block skip.
        //let self_level1_index = self.block_iter.state.level1_iter.current();
        
        if cursor_level0_index == self.block_iter.state.level0_iter.current()
        && cursor_level1_index == self.block_iter.state.level1_iter.current()
        {
            self.data_block_iter.bit_block_iter.zero_first_n(cursor.data_index);
        } else if /* we jumped forward? */
               cursor_level0_index >= self.block_iter.state.level0_iter.current()
            && cursor_level0_index >= self.block_iter.state.level0_iter.current()
        {
            self.data_block_iter = DataBlockIter::empty();
        } else {
            // cursor behind iterator
        }
    }*/

    #[inline]
    fn cursor(&self) -> IndexIterCursor {
        if self.block_iter.state.level0_index == usize::MAX{
            return IndexIterCursor::default();
        }
        
        // Extract level0_index, level1_index from block_start_index
        let (level0_index, level1_index, _) = level_indices::<T::Config>(self.data_block_iter.start_index);
         
        IndexIterCursor{
            block_cursor: BlockIterCursor{ 
                level0_index, 
                // This will actually point to current index, not to next one.
                level1_next_index: level1_index     
            },
            data_next_index: self.data_block_iter.bit_block_iter.current(),
        }        
    }
}

impl<T> Iterator for CachingIndexIter2<T>
where
    T: LevelMasksExt,
{
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // TODO: ?? Still empty blocks ??
        // looping, because BlockIter may return empty DataBlocks.
        loop{
            if let Some(index) = self.data_block_iter.next(){
                return Some(index);
            }

            if let Some(data_block) = self.block_iter.next(){
                self.data_block_iter = data_block.into_iter();
            } else {
                return None;
            }
        }
    }
}