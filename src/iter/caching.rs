use std::mem::{ManuallyDrop, MaybeUninit};

use crate::bit_block::BitBlock;
use crate::bit_queue::BitQueue;
use crate::bitset_interface::{BitSetBase, LevelMasksExt};
use crate::level_indices;

use super::*;

/// Caching block iterator.
///
/// Constructed by [BitSetInterface], or acquired from [CachingIndexIter::as_blocks].
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
/// [BitSetInterface]: crate::BitSetInterface
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
    #[inline]
    pub(crate) fn new(virtual_set: T) -> Self {
        let state = State{
            level0_iter: virtual_set.level0_mask().bits_iter(),
            level1_iter: BitQueue::empty(),
            // usize::MAX - is marker, that we in "intial state".
            // Which means that only level0_iter initialized, and in original state.
            level0_index: usize::MAX,    
        };
        let cache_data = virtual_set.make_cache();
        Self{
            virtual_set,
            state,
            cache_data: ManuallyDrop::new(cache_data),
            level1_blocks: MaybeUninit::uninit()
        }
    }
}


impl<T> BlockIterator for CachingBlockIter<T>
where
    T: LevelMasksExt,
{
    type DataBitBlock = <T::Config as IConfig>::DataBitBlock;  

    #[inline]
    fn cursor(&self) -> BlockIterCursor {
        // "initial state"?
        if self.state.level0_index == usize::MAX /*almost never*/ {
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
    
    #[must_use]
    #[inline]
    fn move_to(mut self, cursor: BlockIterCursor) -> Self{
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
            self.state.level0_index = 1 << <T::Config as IConfig>::DataBitBlock::SIZE_POT_EXPONENT; 
        }

        self
    }
  
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


/// Caching index iterator.
/// 
/// Constructed by [BitSetInterface], or acquired from [CachingBlockIter::as_indices].
/// 
/// Same as [CachingBlockIter] but for indices.
///
/// [BitSetInterface]: crate::BitSetInterface 
pub struct CachingIndexIter<T>
where
    T: LevelMasksExt,
{
    block_iter: CachingBlockIter<T>,
    data_block_iter: DataBlockIter<<T::Config as IConfig>::DataBitBlock>,
}

impl<T> CachingIndexIter<T>
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
}

impl<T> IndexIterator for CachingIndexIter<T>
where
    T: LevelMasksExt,
{
    type BlockIter = CachingBlockIter<T>;

    #[inline]
    fn as_blocks(self) -> Self::BlockIter{
        self.block_iter
    }

    #[must_use]
    #[inline]
    fn move_to(mut self, cursor: IndexIterCursor) -> Self {
        self.block_iter = self.block_iter.move_to(cursor.block_cursor.clone()/*TODO: Make block_cursor Copy*/);
        
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

        self 
    }    

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

impl<T> Iterator for CachingIndexIter<T>
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