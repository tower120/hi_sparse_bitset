use std::marker::PhantomData;
use std::mem;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::ControlFlow;

use crate::bit_block::BitBlock;
use crate::bit_queue::BitQueue;
use crate::bitset_interface::{BitSetBase, LevelMasksIterExt};
use crate::{data_block_start_index, DataBlock, DataBlockIter, level_indices};
use crate::config::Config;
use crate::iter::{BlockCursor, IndexCursor};

/// Caching block iterator.
///
/// Constructed by [BitSetInterface].
///
/// Cache pre-data level block info, making data blocks access faster.
/// This allows to have some additional logic - for example [Reduce] discard 
/// sets with empty level1 blocks.
/// Since only intersection operation produce TrustedHierarchy, which exists in all input sets -
/// all other operations eventually could traverse through empty level blocks across hierarchy.
/// [Reduce] logic - eliminate this effect.
/// 
/// # traverse / for_each
/// 
/// Block [traverse]/[for_each] is up to 25% faster then iteration.
/// 
/// # Empty blocks
/// 
/// Block iterator may occasionally return empty blocks.
/// This is for performance reasons - it is faster to just iterate/traverse empty
/// blocks through, then to add adding additional `is_empty` check in the middle of the loop.
/// 
/// TODO: consider changing this behavior.
///
/// # Memory footprint
///
/// This iterator may store some data in its internal state.
/// Amount of memory used by cache depends on [cache] type.
/// Cache affects only [reduce] operations.
/// 
/// [BitSetInterface]: crate::BitSetInterface
/// [Reduce]: crate::Reduce
/// [cache]: crate::cache
/// [reduce]: crate::reduce()
/// [binary_op]: crate::ops
/// [traverse]: Self::traverse
/// [for_each]: std::iter::Iterator::for_each
pub struct CachingBlockIter<T>
where
    T: LevelMasksIterExt,
{
    virtual_set: T,

    level0_iter: <<T::Conf as Config>::Level0BitBlock as BitBlock>::BitsIter,
    level1_iter: <<T::Conf as Config>::Level1BitBlock as BitBlock>::BitsIter,
    level0_index: usize,

    state: ManuallyDrop<T::IterState>,
    level1_block_data: MaybeUninit<T::Level1BlockData>,
}

impl<T> Clone for CachingBlockIter<T>
where
    T: LevelMasksIterExt + Clone
{
    #[inline]
    fn clone(&self) -> Self {
        let state = self.virtual_set.make_iter_state();
        
        let mut this = Self { 
            virtual_set : self.virtual_set.clone(), 
            level0_iter : self.level0_iter.clone(),
            level1_iter : self.level1_iter.clone(),
            level0_index: self.level0_index,            
            state: ManuallyDrop::new(state),
            level1_block_data: MaybeUninit::uninit()
        };
        
        /*const*/ let have_state = mem::size_of::<T::IterState>() > 0;
        if !have_state {
            // bitwise-copy level1_block_data if have no IterState state.
            
            this.level1_block_data = unsafe{ std::ptr::read(&self.level1_block_data) };
        } else {
            // update level1_block_data otherwise.
            // (because level1_block_data may depends on state)
            
            // Check if level0_index is valid.
            // level0_index can be only invalid in initial state and for "end".
            if this.level0_index < <T::Conf as Config>::Level0BitBlock::size()
            {
                unsafe {
                    // Do not drop level1_block_data, since it was never initialized before.
                    this.virtual_set.init_level1_block_data(
                        &mut this.state,
                        &mut this.level1_block_data,
                        this.level0_index
                    );    
                }
            }            
        }

        this
    }
}

impl<T> CachingBlockIter<T>
where
    T: LevelMasksIterExt,
{
    #[inline]
    pub(crate) fn new(virtual_set: T) -> Self {
        let level0_iter = virtual_set.level0_mask().into_bits_iter(); 
        let state = virtual_set.make_iter_state();
        Self{
            virtual_set,
            
            level0_iter,
            level1_iter: BitQueue::empty(),
            // usize::MAX - is marker, that we in "intial state".
            // Which means that only level0_iter initialized, and in original state.
            level0_index: usize::MAX,    

            state: ManuallyDrop::new(state),
            level1_block_data: MaybeUninit::new(Default::default())
        }
    }
    
    /// Constructs cursor for BlockIterator, with current iterator position.
    /// 
    /// This means that if you [move_to] iterator to cursor, 
    /// iterator will be in the same position as now. IOW - cursor points
    /// to the NEXT element.
    /// 
    /// [move_to]: Self::move_to    
    #[inline]
    pub fn cursor(&self) -> BlockCursor<T::Conf> {
        // "initial state"?
        if self.level0_index == usize::MAX /*almost never*/ {
            return BlockCursor::default();
        }
        
        BlockCursor {
            level0_index     : self.level0_index as u16,
            level1_next_index: self.level1_iter.current() as u16,
            phantom: PhantomData
        }
    }
    
    /// Into index iterator.
    /// 
    /// Index iterator will start iteration from next block.
    #[inline]
    pub fn into_indices(mut self) -> CachingIndexIter<T> {
        let data_block_iter =
            if let Some(data_block) = self.next(){
                data_block.into_iter()
            } else {
                DataBlockIter { 
                    start_index   : usize::MAX, 
                    bit_block_iter: BitQueue::empty() 
                }                
            };
        
        CachingIndexIter{
            block_iter: self,
            data_block_iter
        }
    }  
    
    /// Move iterator to cursor position.
    /// 
    /// Fast O(1) operation.
    #[must_use]
    #[inline]
    pub fn move_to(mut self, cursor: BlockCursor<T::Conf>) -> Self{
        // Reset level0 mask if we not in "initial state"
        if self.level0_index != usize::MAX{
            self.level0_iter = self.virtual_set.level0_mask().into_bits_iter();    
        }
        
        // Mask out level0 mask
        let cursor_level0_index = cursor.level0_index as usize;
        self.level0_iter.zero_first_n(cursor_level0_index);

        if let Some(level0_index) = self.level0_iter.next(){
            self.level0_index = level0_index;
            
            // generate level1 mask, and update cache.
            let level1_mask = unsafe {
                self.level1_block_data.assume_init_drop();
                let (level1_mask, _) = self.virtual_set.init_level1_block_data(
                    &mut self.state,
                    &mut self.level1_block_data,
                    level0_index
                );
                level1_mask
            };
            self.level1_iter = level1_mask.into_bits_iter();
            
            // TODO: can we mask SIMD block directly? 
            // mask out level1 mask, if this is block pointed by cursor
            if level0_index == cursor_level0_index{
                self.level1_iter.zero_first_n(cursor.level1_next_index as usize);
            }
        } else {
            // absolutely empty
            self.level1_iter  = BitQueue::empty();
            self.level0_index = <T::Conf as Config>::DataBitBlock::size(); 
        }

        self
    }

    /// Stable [try_for_each] version.
    /// 
    /// [try_for_each]: std::iter::Iterator::try_for_each
    #[inline]
    pub fn traverse<F>(mut self, mut f: F) -> ControlFlow<()>
    where
        F: FnMut(DataBlock<<T::Conf as Config>::DataBitBlock>) -> ControlFlow<()>    
    {
        // Self have Drop - hence we can't move out values from it.
        // We need level0_iter and level1_iter - we'll ptr::read them instead.
        // It is ok - since they does not participate in Self::Drop.
        // See https://github.com/Jules-Bertholet/rfcs/blob/manuallydrop-deref-move/text/3466-manuallydrop-deref-move.md#rationale-and-alternatives
        
        // compiler SHOULD be able to detect and opt-out this branch away for
        // traverse() after new() call.
        if self.level0_index != usize::MAX{
            let level0_index = self.level0_index;
            
            let level1_iter = unsafe{ std::ptr::read(&self.level1_iter) };
            let ctrl = level1_iter.traverse(
                |level1_index| level1_mask_traverse_fn::<T, _>(
                    level0_index, level1_index, &self.level1_block_data, |b| f(b)
                )
            );
            if ctrl.is_break(){
                return ControlFlow::Break(());
            }
        }

        let level0_iter = unsafe{ std::ptr::read(&self.level0_iter) };
        level0_iter.traverse(
            |level0_index| level0_mask_traverse_fn(
                &self.virtual_set,
                level0_index,
                &mut self.state,
                &mut self.level1_block_data,
                |b| f(b)
            )    
        )
    }    
}

impl<T> Iterator for CachingBlockIter<T>
where
    T: LevelMasksIterExt,
{
    type Item = DataBlock<<T::Conf as Config>::DataBitBlock>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let level1_index = loop {
            if let Some(index) = self.level1_iter.next() {
                break index;
            } else {
                //update level0
                if let Some(index) = self.level0_iter.next() {
                    self.level0_index = index;
                    
                    let level1_mask = unsafe {
                        self.level1_block_data.assume_init_drop();
                        let (level1_mask, _) = 
                            self.virtual_set.init_level1_block_data(
                                &mut self.state,
                                &mut self.level1_block_data,
                                index
                            );
                        level1_mask
                    };

                    self.level1_iter = level1_mask.into_bits_iter();
                } else {
                    return None;
                }
            }
        };

        let data_mask = unsafe {
            T::data_mask_from_block_data(
                self.level1_block_data.assume_init_ref(), level1_index
            )
        };

        let block_start_index =
            data_block_start_index::<<T as BitSetBase>::Conf>(
                self.level0_index, level1_index,
            );

        Some(DataBlock { start_index: block_start_index, bit_block: data_mask })
    }

    #[inline]
    fn for_each<F>(self, mut f: F)
    where
        F: FnMut(Self::Item)
    {
        self.traverse(|block| {
            f(block);
            ControlFlow::Continue(())
        });
    }
}

impl<T> Drop for CachingBlockIter<T>
where
    T: LevelMasksIterExt
{
    #[inline]
    fn drop(&mut self) {
        unsafe{
            self.level1_block_data.assume_init_drop();
            self.virtual_set.drop_iter_state(&mut self.state);
        }
    }
}


/// Caching index iterator.
/// 
/// Constructed by [BitSetInterface], or acquired from [CachingBlockIter::into_indices].
/// 
/// Same as [CachingBlockIter] but for indices.
/// 
/// # traverse / for_each
/// 
/// Index [traverse]/[for_each] is up to 2x faster then iteration.
///
/// [BitSetInterface]: crate::BitSetInterface
/// [traverse]: Self::traverse
/// [for_each]: std::iter::Iterator::for_each
pub struct CachingIndexIter<T>
where
    T: LevelMasksIterExt,
{
    block_iter: CachingBlockIter<T>,
    data_block_iter: DataBlockIter<<T::Conf as Config>::DataBitBlock>,
}

impl<T> Clone for CachingIndexIter<T>
where
    T: LevelMasksIterExt + Clone
{
    #[inline]
    fn clone(&self) -> Self {
        Self{
            block_iter: self.block_iter.clone(),
            data_block_iter: self.data_block_iter.clone(),
        }
    }
}

impl<T> CachingIndexIter<T>
where
    T: LevelMasksIterExt,
{
    #[inline]
    pub(crate) fn new(virtual_set: T) -> Self {
        Self{
            block_iter: CachingBlockIter::new(virtual_set),
            data_block_iter: DataBlockIter{
                // do not calc `start_index` now - will be calculated in 
                // iterator, or in move_to.
                start_index: 0, 
                bit_block_iter: BitQueue::empty(),
            }
        }
    }
    
    /// Move iterator to cursor position.
    /// 
    /// Fast O(1) operation.
    #[must_use]
    #[inline]
    pub fn move_to(mut self, cursor: IndexCursor<T::Conf>) -> Self {
        self.block_iter = self.block_iter.move_to(cursor.block_cursor);
        
        self.data_block_iter = 
        if let Some(data_block) = self.block_iter.next(){
            let mut data_block_iter = data_block.into_iter();
            
            // mask out, if this is block pointed by cursor
            let cursor_block_start_index = data_block_start_index::<T::Conf>(
                cursor.block_cursor.level0_index as usize, 
                cursor.block_cursor.level1_next_index /*this is current index*/ as usize,
            );
            if data_block_iter.start_index == cursor_block_start_index{
                data_block_iter.bit_block_iter.zero_first_n(cursor.data_next_index as usize);
            }
            
            data_block_iter
        } else {
            // absolutely empty
            // point to the end
            DataBlockIter{
                start_index: usize::MAX,
                bit_block_iter: BitQueue::empty(),
            }
        };       

        self 
    }    

    /// Same as [CachingBlockIter::cursor], but for index.
    #[inline]
    pub fn cursor(&self) -> IndexCursor<T::Conf> {
        if self.block_iter.level0_index == usize::MAX{
            return IndexCursor::default();
        }
        
        // Extract level0_index, level1_index from block_start_index
        let (level0_index, level1_index, _) = level_indices::<T::Conf>(self.data_block_iter.start_index);
         
        IndexCursor {
            block_cursor: BlockCursor { 
                level0_index: level0_index as u16, 
                // This will actually point to current index, not to next one.
                level1_next_index: level1_index as u16,
                phantom: PhantomData
            },
            data_next_index: self.data_block_iter.bit_block_iter.current() as u32,
        }        
    }

    /// Stable [try_for_each] version.
    /// 
    /// [try_for_each]: std::iter::Iterator::try_for_each
    #[inline]
    pub fn traverse<F>(mut self, mut f: F) -> ControlFlow<()>
    where
        F: FnMut(usize) -> ControlFlow<()>    
    {
        // See CachingBlockIter::traverse comments.

        if self.block_iter.level0_index != usize::MAX{
            let level0_index = self.block_iter.level0_index;

            // 1. traverse data block
            let ctrl = self.data_block_iter.traverse(|i| f(i));
            if ctrl.is_break(){
                return ControlFlow::Break(());
            }

            // 2. traverse rest of the level1 block
            let level1_iter = unsafe{ std::ptr::read(&self.block_iter.level1_iter) };
            let ctrl = level1_iter.traverse(
                |level1_index| level1_mask_traverse_fn::<T, _>(
                    level0_index, level1_index, &self.block_iter.level1_block_data,
                    |b| b.traverse(|i| f(i))
                )
            );
            if ctrl.is_break(){
                return ControlFlow::Break(());
            }
        }

        let level0_iter = unsafe{ std::ptr::read(&self.block_iter.level0_iter) };
        level0_iter.traverse(
            |level0_index| level0_mask_traverse_fn(
                &self.block_iter.virtual_set,
                level0_index,
                &mut self.block_iter.state,
                &mut self.block_iter.level1_block_data,
                |b| b.traverse(|i| f(i))
            )    
        )        
    }        
}

impl<T> Iterator for CachingIndexIter<T>
where
    T: LevelMasksIterExt,
{
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
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

    #[inline]
    fn for_each<F>(self, mut f: F)
    where
        F: FnMut(Self::Item)
    {
        self.traverse(|index| {
            f(index);
            ControlFlow::Continue(())
        });
    }    
}


#[inline]
fn level1_mask_traverse_fn<S, F>(
    level0_index: usize,
    level1_index: usize,
    level1_block_data: &MaybeUninit<S::Level1BlockData>,
    mut f: F
) -> ControlFlow<()>
where
    S: LevelMasksIterExt, 
    F: FnMut(DataBlock<<S::Conf as Config>::DataBitBlock>) -> ControlFlow<()>
{
    let data_mask = unsafe {
        S::data_mask_from_block_data(level1_block_data.assume_init_ref(), level1_index)
    };
    
    let block_start_index =
        crate::data_block_start_index::<<S as BitSetBase>::Conf>(
            level0_index, level1_index
        );

    f(DataBlock{ start_index: block_start_index, bit_block: data_mask })
}

#[inline]
fn level0_mask_traverse_fn<S, F>(
    set: &S,
    level0_index: usize,
    state: &mut S::IterState,
    level1_blocks: &mut MaybeUninit<S::Level1BlockData>,
    mut f: F
) -> ControlFlow<()>
where
    S: LevelMasksIterExt, 
    F: FnMut(DataBlock<<S::Conf as Config>::DataBitBlock>) -> ControlFlow<()>
{
    let level1_mask = unsafe{
        level1_blocks.assume_init_drop();
        let (level1_mask, _) = 
            set.init_level1_block_data(state, level1_blocks, level0_index);
        level1_mask
    };
    
    level1_mask.traverse_bits(|level1_index|{
        level1_mask_traverse_fn::<S, _>(level0_index, level1_index, level1_blocks, |b| f(b))
    })
}
