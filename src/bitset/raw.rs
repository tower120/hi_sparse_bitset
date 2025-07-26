use std::marker::PhantomData;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ptr::NonNull;
use crate::config::Config;
use crate::{BitBlock, BitSetBase, BitSetInterface, level_indices, DataBlock};
use crate::bitset_interface::{LevelMasks, LevelMasksIterExt};
use crate::bitset::level::{IBlock, Level};
use crate::primitive::Primitive;

pub struct RawBitSet<Conf, Level0Block, Level1Block, LevelDataBlock>
where
    Level0Block: IBlock,
    Level1Block: IBlock,
    LevelDataBlock: IBlock,
{
    pub(crate) level0: Level0Block,
    pub(crate) level1: Level<Level1Block>,
    pub(crate) data  : Level<LevelDataBlock>,
    pub(crate) phantom: PhantomData<Conf>
}

impl<Conf, Level0Block, Level1Block, LevelDataBlock> Clone for RawBitSet<Conf, Level0Block, Level1Block, LevelDataBlock>
where
    Conf: Config,
    Level0Block: IBlock + Clone,
    Level1Block: IBlock + Clone,
    LevelDataBlock: IBlock + Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        Self{
            level0: self.level0.clone(),
            level1: self.level1.clone(),
            data: self.data.clone(),
            phantom: Default::default(),
        }
    }
}

impl<Conf, Level0Block, Level1Block, LevelDataBlock> Default for RawBitSet<Conf, Level0Block, Level1Block, LevelDataBlock>
where
    Conf: Config,
    Level0Block: IBlock,
    Level1Block: IBlock,
    LevelDataBlock: IBlock,
{
    #[inline]
    fn default() -> Self {
        Self{
            level0: Default::default(),
            level1: Default::default(),
            data: Default::default(),
            phantom: PhantomData
        }
    }
}

impl<Conf, Level0Block, Level1Block, LevelDataBlock> FromIterator<usize> for RawBitSet<Conf, Level0Block, Level1Block, LevelDataBlock>
where
    Conf: Config,
    Level0Block: IBlock,
    Level1Block: IBlock,
    LevelDataBlock: IBlock,
{
    fn from_iter<T: IntoIterator<Item=usize>>(iter: T) -> Self {
        let mut this = Self::default();
        for i in iter{
            this.insert(i);
        }
        this
    }
}

impl<Conf, Level0Block, Level1Block, LevelDataBlock, const N: usize> From<[usize; N]> for RawBitSet<Conf, Level0Block, Level1Block, LevelDataBlock>
where
    Conf: Config,
    Level0Block: IBlock,
    Level1Block: IBlock,
    LevelDataBlock: IBlock,
{
    #[inline]
    fn from(value: [usize; N]) -> Self {
        Self::from_iter(value.into_iter())
    }
}

impl<Conf, Level0Block, Level1Block, LevelDataBlock, B> From<B> for RawBitSet<Conf, Level0Block, Level1Block, LevelDataBlock>
where
    B: BitSetInterface<Conf = Conf>,
    Conf: Config<DataBitBlock = LevelDataBlock::Mask>,
    Level0Block: IBlock,
    Level1Block: IBlock,
    LevelDataBlock: IBlock,
{
    #[inline]
    fn from(bitset: B) -> Self {
        /*if B::TRUSTED_HIERARCHY{
            todo!("optimized special case with hierarchies + prealocated space")
        }*/
        
        // number of blocks in each level unknown.
        // insert block by block.
        // We only know that blocks come in order.
        let mut this = Self::default();
        let mut global_level1_index = usize::MAX;
        let mut level1_block_ptr: Option<NonNull<Level1Block>> = None;
        bitset.block_iter().for_each(|block|{
            // block can be empty
            if block.is_empty(){
                return;
            }
            
            // TODO: block_iter could just return these
            let (inner_level0_index, inner_level1_index, _) = Self::level_indices(block.start_index);
            
            // block.start_index / 2^Conf::DataBitBlock::SIZE_POT_EXPONENT
            let current_level1_block_index = block.start_index >> Conf::DataBitBlock::SIZE_POT_EXPONENT;
            if current_level1_block_index != global_level1_index {
                global_level1_index = current_level1_block_index;
                
                // 1. Level0
                let level1_block_index = unsafe{
                    this.level0.get_or_insert(inner_level0_index, ||{
                        let block_index = this.level1.insert_block();
                        Primitive::from_usize(block_index)
                    })
                }.as_usize();

                // 2. Level1
                level1_block_ptr = Some(NonNull::from(
                    unsafe{
                        this.level1.blocks_mut().get_unchecked_mut(level1_block_index)
                    }
                ));
            }

            // 3. Data Level
            unsafe{
                // Make data block.
                let mut data_block = LevelDataBlock::default();
                *data_block.mask_mut() = block.bit_block;
                
                // Push block to data level. 
                let data_block_index = this.data.push_block(data_block);

                // Register block's index in level1.
                level1_block_ptr.unwrap_unchecked().as_mut()
                    .insert_unchecked(inner_level1_index, Primitive::from_usize(data_block_index));
            }
        });
        this
    }
}

impl<Conf, Level0Block, Level1Block, LevelDataBlock> RawBitSet<Conf, Level0Block, Level1Block, LevelDataBlock>
where
    Conf: Config,
    Level0Block: IBlock,
    Level1Block: IBlock,
    LevelDataBlock: IBlock,
{
    #[inline]
    fn level_indices(index: usize) -> (usize/*level0*/, usize/*level1*/, usize/*data*/){
        level_indices::<Conf>(index)
    }
    
    /// Max usize, [BitSet] with this `Config` can hold.
    /// 
    /// [BitSet]: crate::BitSet
    #[inline]
    pub const fn max_capacity() -> usize {
        Conf::MAX_CAPACITY
    }      
    
    #[inline]
    fn is_in_range(index: usize) -> bool{
        index < Self::max_capacity()
    }

    /// # Safety
    /// 
    /// indices are not checked
    #[inline]
    unsafe fn get_block_indices(&self, level0_index: usize, level1_index: usize)
        -> (usize/*level1_block_index*/, usize/*data_block_index*/)
    {
        let level1_block_index = self.level0.get_or_zero(level0_index).as_usize();
        let level1_block = self.level1.blocks().get_unchecked(level1_block_index);
        let data_block_index = level1_block.get_or_zero(level1_index).as_usize();
        (level1_block_index, data_block_index)
    }
    
    /// # Safety
    /// 
    /// indices are not checked
    #[inline]
    unsafe fn get_or_insert_data_block(&mut self, level0_index: usize, level1_index: usize) 
        -> &mut LevelDataBlock 
    {
        // 1. Level0
        let level1_block_index = 
            self.level0.get_or_insert(level0_index, ||{
                let block_index = self.level1.insert_block();
                Primitive::from_usize(block_index)
            }).as_usize();

        // 2. Level1
        let data_block_index = { 
            let level1_block = self.level1.blocks_mut().get_unchecked_mut(level1_block_index);
            level1_block.get_or_insert(level1_index, ||{
                let block_index = self.data.insert_block();
                Primitive::from_usize(block_index)
            }).as_usize()
        };

        // 3. Data block
        self.data.blocks_mut().get_unchecked_mut(data_block_index)
    }
    
    /// # Safety
    ///
    /// Will panic, if `index` is out of range.
    pub fn insert(&mut self, index: usize){
        assert!(Self::is_in_range(index), "{index} is out of index range!");

        // That's indices to next level
        let (level0_index, level1_index, data_index) = Self::level_indices(index);

        unsafe{
            let data_block = self.get_or_insert_data_block(level0_index, level1_index);
            data_block.mask_mut().set_bit::<true>(data_index);
        }
    }

    // TODO: test and document
    /// # Safety
    ///
    /// Will panic, if `index` is out of range.
    pub fn insert_block(&mut self, block: DataBlock<LevelDataBlock::Mask>) {
        if block.is_empty() {
            return;
        }
        
        // does block end is in range?
        assert!(
            Self::is_in_range(block.start_index + LevelDataBlock::Mask::size()), 
            "{:?} is out of index range!", block
        );

        // That's indices to next level
        let (level0_index, level1_index, _) = Self::level_indices(block.start_index);

        unsafe{
            let data_block = self.get_or_insert_data_block(level0_index, level1_index);
            *data_block.mask_mut() |= block.bit_block; 
        }
    }    
    
    /// Returns false if index is invalid/not in bitset.
    pub fn remove(&mut self, index: usize) -> bool {
        if !Self::is_in_range(index){
            return false;
        }

        // 1. Resolve indices
        let (level0_index, level1_index, data_index) = Self::level_indices(index);
        let (level1_block_index, data_block_index) = unsafe {
            self.get_block_indices(level0_index, level1_index)
        };
        if data_block_index == 0 {
            return false;
        }

        unsafe {
            // 2. Get Data block and set bit
            let data_block = self.data.blocks_mut().get_unchecked_mut(data_block_index);
            let existed = data_block.mask_mut().set_bit::<false>(data_index);
            
            // 3. Remove free blocks
            if data_block.is_empty(){
                // remove data block
                self.data.remove_empty_block_unchecked(data_block_index);

                // remove pointer from level1
                let level1_block = self.level1.blocks_mut().get_unchecked_mut(level1_block_index);
                level1_block.remove_unchecked(level1_index);

                if level1_block.is_empty(){
                    // remove level1 block
                    self.level1.remove_empty_block_unchecked(level1_block_index);

                    // remove pointer from level0
                    self.level0.remove_unchecked(level0_index);
                }
            }
            existed
        }
    }
}

impl<Conf, Level0Block, Level1Block, LevelDataBlock> BitSetBase 
for 
    RawBitSet<Conf, Level0Block, Level1Block, LevelDataBlock>
where
    Conf: Config,
    Level0Block: IBlock,
    Level1Block: IBlock,
    LevelDataBlock: IBlock,
{
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = true;
}

impl<Conf, Level0Block, Level1Block, LevelDataBlock> LevelMasks 
for 
    RawBitSet<Conf, Level0Block, Level1Block, LevelDataBlock>
where
    Conf: Config,
    Level0Block: IBlock<Mask = Conf::Level0BitBlock>,
    Level1Block: IBlock<Mask = Conf::Level1BitBlock>,
    LevelDataBlock: IBlock<Mask = Conf::DataBitBlock>
{
    #[inline]
    fn level0_mask(&self) -> Conf::Level0BitBlock {
        *self.level0.mask()
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize) -> Conf::Level1BitBlock {
        let level1_block_index = self.level0.get_or_zero(level0_index).as_usize();
        let level1_block = self.level1.blocks().get_unchecked(level1_block_index);
        *level1_block.mask()
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize) -> Conf::DataBitBlock {
        let (_, data_block_index) = self.get_block_indices(level0_index, level1_index);
        let data_block = self.data.blocks().get_unchecked(data_block_index);
        *data_block.mask()
    }
}

impl<Conf, Level0Block, Level1Block, LevelDataBlock> LevelMasksIterExt 
for 
    RawBitSet<Conf, Level0Block, Level1Block, LevelDataBlock>
where
    Conf: Config,
    Level0Block: IBlock<Mask = Conf::Level0BitBlock>,
    Level1Block: IBlock<Mask = Conf::Level1BitBlock>,
    LevelDataBlock: IBlock<Mask = Conf::DataBitBlock>
{
    /// Points to elements in heap. Guaranteed to be stable.
    /// This is just plain pointers with null in default:
    /// `(*const LevelDataBlock<Conf>, *const Level1Block<Conf>)`
    type Level1BlockData = (
        Option<NonNull<LevelDataBlock>>,  /* data array pointer */
        Option<NonNull<Level1Block>>      /* block pointer */
    );

    type IterState = ();
    fn make_iter_state(&self) -> Self::IterState { () }
    unsafe fn drop_iter_state(&self, _: &mut ManuallyDrop<Self::IterState>) {}

    #[inline]
    unsafe fn init_level1_block_data(
        &self,
        _: &mut Self::IterState,
        level1_block_data: &mut MaybeUninit<Self::Level1BlockData>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool){
        let level1_block_index = self.level0.get_or_zero(level0_index);
        let level1_block = self.level1.blocks().get_unchecked(level1_block_index.as_usize());
        level1_block_data.write(
            (
                Some(NonNull::new_unchecked(self.data.blocks().as_ptr() as *mut _)),
                Some(NonNull::from(level1_block))
            )
        );
        (*level1_block.mask(), !level1_block_index.is_zero())
    }

    #[inline]
    unsafe fn data_mask_from_block_data(
        level1_blocks: &Self::Level1BlockData, level1_index: usize
    ) -> Conf::DataBitBlock {
        let array_ptr = level1_blocks.0.unwrap_unchecked().as_ptr().cast_const();
        let level1_block = level1_blocks.1.unwrap_unchecked().as_ref();

        let data_block_index = level1_block.get_or_zero(level1_index);
        let data_block = &*array_ptr.add(data_block_index.as_usize());
        *data_block.mask()
    }
}
