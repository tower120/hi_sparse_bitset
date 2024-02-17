use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::{ControlFlow, Range, RangeBounds, RangeInclusive};
use std::ops::ControlFlow::Continue;
use std::ptr::NonNull;
use crate::{BitBlock, BitSet, BitSetBase, DataBlock, internals, Level0Block, Level1, Level1Block, level_indices, LevelData, LevelDataBlock};
use crate::bit_block::BitBlockFull;
use crate::bit_utils::{fill_bits_array_from_unchecked, fill_bits_array_to_unchecked, fill_bits_array_unchecked, slice_bits_array_unchecked, traverse_one_bits_array};
use crate::bitset_interface::{LevelMasks, LevelMasksIterExt};
use crate::block::Block;
use crate::config::{Config, max_addressable_index};
use crate::level::Level;
use crate::primitive::Primitive;

// TODO: add description image 
//
// # Implementation details
//
// At level1 and data level allocated one empty block at index 0,
// and one full block at index 1. As soon as block become full - 
// it replaced with preallocated block 1. 
// Full block allows to have more packed bitset,
// and do contiguous fill algorithmically faster. 
pub struct BitSetRanges<Conf: Config>{
    bitset: BitSet<Conf>,
    // TODO: type in Config
    level1_full_data_block_counters: [u8; 256] 
}

impl<Conf: Config> Clone for BitSetRanges<Conf>{
    #[inline]
    fn clone(&self) -> Self {
        Self{ 
            bitset: self.bitset.clone(),
            level1_full_data_block_counters: self.level1_full_data_block_counters.clone()
        }
    }
}

impl<Conf: Config> Default for BitSetRanges<Conf>
where
    Conf::Level1BitBlock: BitBlockFull,
    Conf::DataBitBlock: BitBlockFull
{
    #[inline]
    fn default() -> Self {
        let bitset = BitSet{
            level0: Block::empty(),
            level1: Level::new(vec![Block::empty(), Block::full()]),
            data  : Level::new(vec![Block::empty(), Block::full()]),
        };
        let level1_full_block_counters = unsafe{ MaybeUninit::zeroed().assume_init() };
        Self{
            bitset,
            level1_full_data_block_counters: level1_full_block_counters
        }
    }
}

impl<Conf: Config> BitSetRanges<Conf>
where
    Conf::Level1BitBlock: BitBlockFull,
    Conf::DataBitBlock: BitBlockFull
{
    #[inline]
    pub fn new() -> Self{
        Default::default()
    }
    
    #[inline]
    pub const fn max_capacity() -> usize {
        // We occupy two blocks at each level, except root.
        max_addressable_index::<Conf>()
            - (1 << Conf::Level1BitBlock::SIZE_POT_EXPONENT)*2
            - (1 << Conf::DataBitBlock::SIZE_POT_EXPONENT)*2
    }
    
    #[inline]
    fn is_in_range(index: usize) -> bool{
        index < Self::max_capacity()
    }
    
    #[inline]
    fn remove_data_block(data: &mut LevelData<Conf>, data_block_index: usize){
        let data_block = unsafe{
            data.blocks_mut().get_unchecked_mut(data_block_index)
        };
        *data_block = Block::empty();
        unsafe{
            data.remove_empty_block_unchecked(data_block_index);
        }
    }
    
    fn clear_level1_block(&mut self, level1_block_index: usize){
        let level1_block = unsafe{
            self.bitset.level1.blocks_mut().as_mut().get_unchecked_mut(level1_block_index)
        };
        
        // I. Clear child data blocks.
        use ControlFlow::*;
        level1_block.mask.traverse_bits(|i|{
            let data_block_index_ref = unsafe{
                level1_block.block_indices.as_mut().get_unchecked_mut(i)
            }; 
            let data_block_index = data_block_index_ref.as_usize();
            
            // 1. free data block
            Self::remove_data_block(&mut self.bitset.data, data_block_index);
            
            // 2. replace its level1 index-pointer with 0.
            *data_block_index_ref = Primitive::from_usize(0);
            
            Continue(())
        });
        
        // II. Clear level1 block mask and counter
        unsafe{
            level1_block.mask = BitBlock::zero();
            *self.level1_full_data_block_counters.get_unchecked_mut(level1_block_index) = Primitive::from_usize(0);
        }
    }
    
    #[inline]
    fn try_pack_full_level1block(
        &mut self,
        in_block_level0_index: usize, level1_block_index: usize, mut level1_block: NonNull<Level1Block<Conf>>
    ) -> bool {    
        let counter = unsafe{
            self.level1_full_data_block_counters.get_unchecked_mut(level1_block_index)
        };

        // try to pack whole level1 block 
        if *counter as usize == Conf::Level1BitBlock::size(){
            *counter = 0;

            // make level1_block empty. Ala *level1_block = Block::empty()
            unsafe {
                let level1_block = level1_block.as_mut();
                level1_block.mask = BitBlock::zero();
                level1_block.block_indices.as_mut().fill(Primitive::ZERO);
            }
            
            // remove level1_block
            unsafe{
                self.bitset.level1.remove_empty_block_unchecked(level1_block_index);
                *self.bitset.level0.block_indices.as_mut().get_unchecked_mut(in_block_level0_index) = Primitive::from_usize(1);
            }
            true
        } else {
            false    
        }
    }
    
    #[inline]
    fn try_pack_full_datablock(
        &mut self,
        level1_block_index: usize, mut level1_block: NonNull<Level1Block<Conf>>, 
        in_block_level1_index: usize,
        data_block_index: usize, data_block: NonNull<LevelDataBlock<Conf>>
    ) -> bool {
        let data_block = unsafe{data_block.as_ref()};
        
        if data_block_index == 1
        || !data_block.is_full() {
            return false;
        }
        
        // replace data block with "full"
        unsafe{
            // 1. at data level - make block empty, and remove
            Self::remove_data_block(&mut self.bitset.data, data_block_index);
            
            // 2. at level1 - change pointer to "full".
            *level1_block.as_mut().block_indices.as_mut().get_unchecked_mut(in_block_level1_index) = Primitive::from_usize(1);
        }        
        
        // increase counter
        {
            let counter = unsafe{
                self.level1_full_data_block_counters.get_unchecked_mut(level1_block_index)
            };
            *counter += 1;
        }
        true
    }
    
    // should be internals of fill_level1_range
    #[inline]
    fn fill_data_block(
        &mut self, 
        level1_block_index: usize, level1_block: NonNull<Level1Block<Conf>>,
        in_block_level1_index: usize, 
        f: impl FnOnce(&mut [u64]))
    {
        let (data_block_index, mut data_block) = unsafe {
            self.bitset.get_or_insert_datablock(level1_block, in_block_level1_index)
        };
        
        if data_block_index == 1 {
            // block already full
            return;
        }
        
        unsafe{
            f(data_block.as_mut().mask.as_array_mut());
        }            
        
        self.try_pack_full_datablock(
            level1_block_index, level1_block, 
            in_block_level1_index, 
            data_block_index, data_block
        );
    }
    
    fn fill_level1_range(
        &mut self,
        in_block_level0_index: usize,
        first_level1_index: usize, first_data_index: usize,
        last_level1_index : usize, last_data_index : usize,
    ){
        let (level1_block_index, mut level1_block) = unsafe {
            self.bitset.get_or_insert_level1block(in_block_level0_index)
        };
        if level1_block_index == 1{
            // already full
            return;
        }
        
        let full_leftest_data  = first_data_index == 0;
        let full_rightest_data = last_data_index == LevelDataBlock::<Conf>::size() - 1;
        
        // I. Coarse fill data blocks
        'coarse: {unsafe{
            // let range = range_start..range_end;
            let range_start = first_level1_index + !full_leftest_data as usize;
            let range_end   = last_level1_index  + full_rightest_data as usize;
            if range_start >= range_end {
                break 'coarse;
            }
            let range_len = range_end - range_start;
            
            // 1. remove all non-fixed data_blocks in range 
            {
                let mut mask = level1_block.as_ref().mask;
                let (offset, mask) = slice_bits_array_unchecked(
                    mask.as_array_mut(), 
                    range_start..=range_end-1
                );
                
                traverse_one_bits_array(mask, |index|{
                    let index = offset + index;
                    let data_block_index = level1_block.as_ref()
                                           .block_indices.as_ref()
                                           .get_unchecked(index).as_usize();
                    
                    // remove data_block
                    // if this is not fixed block
                    if data_block_index > 1{
                        Self::remove_data_block(&mut self.bitset.data, data_block_index);
                    }
                    
                    Continue(())
                });
            }
            
            // 2. fill all index-pointers with 1 in range
            //level1_block.block_indices_mut()[range].fill(Primitive::from_usize(1));
            std::slice::from_raw_parts_mut(
                level1_block.as_mut().block_indices.as_mut().as_mut_ptr().add(range_start),
                range_len
            ).fill(Primitive::from_usize(1));
            
            // 3. set full counter
            *self.level1_full_data_block_counters.get_unchecked_mut(level1_block_index) = 
                Primitive::from_usize(range_len);
        }}
        
        // II. Fine fill edge data blocks.
        if first_level1_index == last_level1_index{
            self.fill_data_block(level1_block_index, level1_block, 
                first_level1_index, 
                |bits| unsafe{ fill_bits_array_unchecked::<true, _>(
                    bits, first_data_index..=last_data_index
                ) }
            );
        } else {
            self.fill_data_block(level1_block_index, level1_block, 
                first_level1_index, 
                |bits| unsafe{ fill_bits_array_from_unchecked::<true, _>(
                    bits, first_data_index..
                ) }
            );
            self.fill_data_block(level1_block_index, level1_block, 
                last_level1_index, 
                |bits| unsafe{ fill_bits_array_to_unchecked::<true, _>(
                    bits, ..=last_data_index
                ) }
            );
        }
        
        // III. Update level1 mask
        unsafe{
            fill_bits_array_unchecked::<true, _>(
                level1_block.as_mut().mask.as_array_mut(),
                first_level1_index..=last_level1_index
            );
        }
        
        // IV. Try to replace whole block with static "filled".
        self.try_pack_full_level1block(in_block_level0_index, level1_block_index, level1_block);
    }
    
    // TODO: RangeBounds
    /// # Complexity
    /// 
    /// O(N) + O(J), where: 
    /// * N - is amount of data blocks that need to be freed, which 
    /// roughly equals to the amount of ranges in container, that `range` intersects.
    /// * J - is amount of block-pointers (at any level) that need to be redirected(changed) to
    /// "full" block. Which virtually is just filling u8 array slice with 1. 
    /// This number cannot be greater than level0 blocks capacity (128 for 128bit bitset).
    pub fn insert_range(&mut self, range: RangeInclusive<usize>){
        let (first_index, last_index) = range.into_inner();
        /*let first_index = range.start;
        let last_index  = range.end - 1;*/        
        assert!(Self::is_in_range(last_index), "range out of range!");
        
        let (
            first_level0_index, 
            first_level1_index, 
            first_data_index
        ) = level_indices::<Conf>(first_index);
        let (
            last_level0_index, 
            last_level1_index, 
            last_data_index
        ) = level_indices::<Conf>(last_index);
        
        let full_leftest_data   = first_data_index == 0;
        let full_leftest_level1 = (first_level1_index == 0) & full_leftest_data;
        
        let full_rightest_data   = last_data_index == LevelDataBlock::<Conf>::size() - 1;
        let full_rightest_level1 = (last_level1_index == Level1Block::<Conf>::size() - 1) 
                                 & full_rightest_data; 
        
        // Coarse fill level1 blocks.
        for level0_index in 
            first_level0_index + !full_leftest_level1 as usize 
            ..
            last_level0_index  + full_rightest_level1 as usize
        {
            let level1_block_index = unsafe {
                let index_ref = self.bitset.level0.block_indices.as_mut().get_unchecked_mut(level0_index);
                let index = index_ref.as_usize();
                
                // replace level0 index with 1
                *index_ref = Primitive::from_usize(1);
                
                index
            };
            
            // remove non-fixed block. 
            if level1_block_index > 1{
                self.clear_level1_block(level1_block_index);
                unsafe{
                    self.bitset.level1.remove_empty_block_unchecked(level1_block_index);
                }
            }
        }
        
        // fill level0 mask
        unsafe{
            fill_bits_array_unchecked::<true, _>(
                self.bitset.level0.mask.as_array_mut(),
                first_level0_index..=last_level0_index
            )
        }
        
        if first_level0_index == last_level0_index{
            self.fill_level1_range(
                first_level0_index,
                first_level1_index, first_data_index,
                last_level1_index , last_data_index ,
            );
        } else {
            self.fill_level1_range(
                first_level0_index,
                first_level1_index, first_data_index,
                Level1Block::<Conf>::size() - 1, LevelDataBlock::<Conf>::size() - 1
            );
            self.fill_level1_range(
                last_level0_index,
                0, 0,
                last_level1_index,  last_data_index,
            );
        }
    }
    
    pub fn insert(&mut self, index: usize){
        assert!(Self::is_in_range(index), "index out of range!");
        
        // edit hierarchy as usual, even for "full" block
        let (
            in_block_level0_index,
            level1_block_index, level1_block, in_block_level1_index,
            data_block_index, data_block, in_block_data_index, 
            mutated_primitive
        ) = unsafe {
            self.bitset.insert_impl(index)
        };
        
        if mutated_primitive == u64::MAX {   // fast check for just mutated part of bitblock
            let data_packed = self.try_pack_full_datablock(
                level1_block_index, level1_block, in_block_level1_index,
                data_block_index  , data_block
            );
            
            if data_packed {
                self.try_pack_full_level1block(in_block_level0_index, level1_block_index, level1_block);
            }
        }
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if !Self::is_in_range(index){
            return false;
        }
        
        let (in_block_level0_index, in_block_level1_index, in_block_data_index) = level_indices::<Conf>(index);
        let (mut level1_block_index, data_block_index) = self.bitset.get_block_indices(in_block_level0_index, in_block_level1_index);
        if data_block_index == 0{
            return false;
        }
        // try unpack full block
        if data_block_index == 1{
            if level1_block_index == 1{
                // insert filled level1 block, and set level0's pointer to it.
                level1_block_index = self.bitset.level1.insert_block(Block::full());
                unsafe{
                    // update pointer
                    *self.bitset.level0.block_indices.as_mut().get_unchecked_mut(in_block_level0_index) = Primitive::from_usize(level1_block_index);
                    // reset counter
                    *self.level1_full_data_block_counters.get_unchecked_mut(level1_block_index) = Conf::Level1BitBlock::size() as _;
                }
            }
            // decrease level1 block full-datablock counter
            let counter = unsafe{
                self.level1_full_data_block_counters.get_unchecked_mut(level1_block_index)
            };
            *counter -= 1;
            
            
            // make new data block
            let new_data_block_index = {
                let mut block: LevelDataBlock<Conf> = Block::full();
                // we will not run remove_impl, so just directly remove bit
                block.mask.set_bit::<false>(in_block_data_index);
                self.bitset.data.insert_block(block)
            };
            
            // point to it from level1 block
            unsafe{
                let level1_block = self.bitset.level1.blocks_mut().get_unchecked_mut(level1_block_index);
                *level1_block.block_indices.as_mut().get_unchecked_mut(in_block_level1_index) = Primitive::from_usize(new_data_block_index);
            }
            
            return true;
        }
 
        // remove as usual
        unsafe{
            self.bitset.remove_impl(
                in_block_level0_index, in_block_level1_index, in_block_data_index,
                level1_block_index, data_block_index
            )
        }
    }
}

impl<Conf: Config> FromIterator<usize> for BitSetRanges<Conf>
where
    Conf::Level1BitBlock: BitBlockFull,
    Conf::DataBitBlock: BitBlockFull
{
    fn from_iter<I: IntoIterator<Item=usize>>(iter: I) -> Self {
        let mut this = Self::default();
        for i in iter{
            this.insert(i);
        }
        this
    }
}

impl<Conf: Config, const N: usize> From<[usize; N]> for BitSetRanges<Conf>
where
    Conf::Level1BitBlock: BitBlockFull,
    Conf::DataBitBlock: BitBlockFull
{
    fn from(value: [usize; N]) -> Self {
        Self::from_iter(value.into_iter())
    }
}

impl<Conf: Config> BitSetBase for BitSetRanges<Conf>{
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = true;
}

impl<Conf: Config> LevelMasks for BitSetRanges<Conf>{
    #[inline]
    fn level0_mask(&self) -> <Self::Conf as Config>::Level0BitBlock {
        self.bitset.level0_mask()
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize) -> <Self::Conf as Config>::Level1BitBlock {
        self.bitset.level1_mask(level0_index)
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize) -> <Self::Conf as Config>::DataBitBlock {
        self.bitset.data_mask(level0_index, level1_index)
    }
}

impl<Conf: Config> LevelMasksIterExt for BitSetRanges<Conf>{
    type IterState = <BitSet<Conf> as LevelMasksIterExt>::IterState;
    type Level1BlockData = <BitSet<Conf> as LevelMasksIterExt>::Level1BlockData;

    #[inline]
    fn make_iter_state(&self) -> Self::IterState {
        self.bitset.make_iter_state()
    }

    #[inline]
    unsafe fn drop_iter_state(&self, state: &mut ManuallyDrop<Self::IterState>) {
        self.bitset.drop_iter_state(state)
    }

    #[inline]
    unsafe fn init_level1_block_data(&self, state: &mut Self::IterState, level1_block_data: &mut MaybeUninit<Self::Level1BlockData>, level0_index: usize) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        self.bitset.init_level1_block_data(state, level1_block_data, level0_index)
    }

    #[inline]
    unsafe fn data_mask_from_block_data(level1_block_data: &Self::Level1BlockData, level1_index: usize) -> <Self::Conf as Config>::DataBitBlock {
        BitSet::<Conf>::data_mask_from_block_data(level1_block_data, level1_index)
    }
}

internals::impl_bitset!(impl<Conf> for ref BitSetRanges<Conf> where Conf: Config);

#[cfg(test)]
mod test{
    use itertools::{assert_equal, Itertools};
    use crate::{BitSetRanges, config};

    #[test]
    fn fill_test(){
        let range = 0..20_000;
        let mut bitset: BitSetRanges<config::_64bit> = range.clone().collect();
        assert_equal(&bitset, range.clone());
        
        // overwrite
        for i in range.clone(){
            bitset.insert(i);    
        }
        assert_equal(&bitset, range.clone());
        
        // remove
        for i in range.clone(){
            bitset.remove(i);    
        }        
        println!("{:?}", &bitset);
        assert_equal(&bitset, []);
        
        // try remove again
        for i in range.clone(){
            bitset.remove(i);    
        }        
        assert_equal(&bitset, []);
        
        for i in range.clone(){
            bitset.insert(i);    
        }
        assert_equal(&bitset, range.clone());
        
        // remove half
        for i in 0..10_000{
            bitset.remove(i);    
        }
        assert_equal(&bitset, 10_000..20_000);

        // insert half
        for i in 0..10_000{
            bitset.insert(i);    
        }
        assert_equal(&bitset, range.clone());
    }
    
    // TODO: fuzzy test
    #[test]
    fn range_insert_test(){
        // left+coarse+right
        {
            let range = 34..=4096*2+18;
            let mut bitset: BitSetRanges<config::_64bit> = Default::default();
            bitset.insert_range(range.clone());
            assert_equal(&bitset, range.clone());
        }         
        
        // no level0 coarse
        {
            let range = 34..=751;
            let mut bitset: BitSetRanges<config::_64bit> = Default::default();
            bitset.insert_range(range.clone());
            println!("{:?}", bitset.iter().collect_vec());
            assert_equal(&bitset, range.clone());
        }  
        
        // right+coarse
        {
            let range = 0..=4096*2+38;
            let mut bitset: BitSetRanges<config::_64bit> = Default::default();
            bitset.insert_range(range.clone());
            assert_equal(&bitset, range.clone());
        }
        
        // left+coarse
        {
            let range = 34..=4096*2-1;
            let mut bitset: BitSetRanges<config::_64bit> = Default::default();
            bitset.insert_range(range.clone());
            assert_equal(&bitset, range.clone());
        }
        
        // coarse
        {
            let range = 4096..=4096*2-1;
            let mut bitset: BitSetRanges<config::_64bit> = Default::default();
            bitset.insert_range(range.clone());
            assert_equal(&bitset, range.clone());
        }
    }
}