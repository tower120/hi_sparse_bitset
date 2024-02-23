use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::{ControlFlow, Range, RangeBounds, RangeInclusive};
use std::ops::ControlFlow::Continue;
use std::ptr::NonNull;
use crate::{BitBlock, BitSet, BitSetBase, DataBlock, internals, Level0Block, Level1, Level1Block, level_indices, LevelData, LevelDataBlock};
use crate::bit_block::BitBlockFull;
use crate::bit_utils::{fill_bits_array_from_unchecked, fill_bits_array_to_unchecked, fill_bits_array_unchecked, traverse_one_bits_array_range_unchecked};
use crate::bitset_interface::{LevelMasks, LevelMasksIterExt};
use crate::block::Block;
use crate::config::{Config, max_addressable_index};
use crate::level::Level;
use crate::primitive::Primitive;

struct BoolConst<const V: bool>;

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
    bitset: BitSet<Conf> 
}

impl<Conf: Config> Clone for BitSetRanges<Conf>{
    #[inline]
    fn clone(&self) -> Self {
        Self{ 
            bitset: self.bitset.clone(),
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
        Self{ bitset }
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
    
    #[inline]
    fn remove_level1_block(level1: &mut Level1<Conf>, data: &mut LevelData<Conf>, level1_block_index: usize){
        Self::clear_level1_block(level1, data, level1_block_index);
        unsafe{
            level1.remove_empty_block_unchecked(level1_block_index);
        }        
    }
    
    #[inline]
    fn clear_level1_block(level1: &mut Level1<Conf>, data: &mut LevelData<Conf>, level1_block_index: usize){
        let level1_block = unsafe{
            level1.blocks_mut().as_mut().get_unchecked_mut(level1_block_index)
        };
        
        // I. Clear child data blocks.
        use ControlFlow::*;
        // TODO:
        //let mask = level1_block.mask & !level1_block.full_mask;  
        level1_block.mask.traverse_bits(|i|{
            let data_block_index_ref = unsafe{
                level1_block.block_indices.as_mut().get_unchecked_mut(i)
            }; 
            let data_block_index = data_block_index_ref.as_usize();
            
            // TODO: remove this check (separate loop)
            if data_block_index > 1{
                // 1. free data block
                Self::remove_data_block(data, data_block_index);
            }    

            // 2. replace its level1 index-pointer with 0.
            *data_block_index_ref = Primitive::from_usize(0);
            
            Continue(())
        });
        
        // II. Clear level1 block masks
        {
            level1_block.mask = BitBlock::zero();
            level1_block.full_mask = BitBlock::zero();
        }
    }
    
    // TOOD: remove
    #[deprecated = "use unpack_full_level1block()"]
    #[inline]
    fn insert_full_level1block(&mut self, in_block_level0_index: usize) -> usize {
        // insert filled level1 block, and set level0's pointer to it.
        let level1_block_index = self.bitset.level1.insert_block(Block::full());
        unsafe{
            // update pointer
            *self.bitset.level0.block_indices.as_mut().get_unchecked_mut(in_block_level0_index) = Primitive::from_usize(level1_block_index);
        }                
        level1_block_index
    }
    
    // TODO: remove
    #[deprecated = "use unpack_full_datablock()"]
    #[inline]
    fn insert_datablock(
        &mut self,
         datablock: LevelDataBlock<Conf>,
         mut level1_block: NonNull<Level1Block<Conf>>,
         in_block_level1_index: usize,
    ) -> usize {
        let new_data_block_index = self.bitset.data.insert_block(datablock);
        
        // point to it from level1 block
        unsafe{
            //let level1_block = self.bitset.level1.blocks_mut().get_unchecked_mut(level1_block_index);
            let level1_block = level1_block.as_mut();
            *level1_block.block_indices.as_mut().get_unchecked_mut(in_block_level1_index) = Primitive::from_usize(new_data_block_index);
        }
        new_data_block_index   
    }
    
    /// # Safety
    /// 
    /// * returned block should not be full in the end.
    /// * index must point to full block.
    #[inline]
    unsafe fn unpack_full_level1block(&mut self, in_block_level0_index: usize) 
        -> usize /*datablock_index*/ 
    {
        self.bitset.level0.full_mask.set_bit::<false>(in_block_level0_index);
        self.insert_full_level1block(in_block_level0_index)
    }
    
    #[inline]
    fn try_pack_full_level1block(
        &mut self,
        in_block_level0_index: usize, level1_block_index: usize, mut level1_block: NonNull<Level1Block<Conf>>
    ) -> bool {    
        debug_assert!(level1_block_index != 1);

        // try to pack whole level1 block 
        let level1_block = unsafe { level1_block.as_mut() };
        if level1_block.full_mask.is_full(){
            level1_block.mask = BitBlock::zero();
            level1_block.full_mask = BitBlock::zero();
            level1_block.block_indices.as_mut().fill(Primitive::ZERO);
            
            // remove level1_block
            unsafe{
                self.bitset.level1.remove_empty_block_unchecked(level1_block_index);
                *self.bitset.level0.block_indices.as_mut().get_unchecked_mut(in_block_level0_index) = Primitive::from_usize(1);
            }
            
            // update level0 full_mask
            self.bitset.level0.full_mask.set_bit::<true>(in_block_level0_index);
            
            true
        } else {
            false    
        }
    }
    
    /// # Safety
    /// 
    /// * returned block should not be full in the end.
    /// * index must point to full block.
    #[inline]
    fn unpack_full_datablock(
        &mut self,
        mut level1_block: NonNull<Level1Block<Conf>>,
        in_block_level1_index: usize,
    ) -> usize /*datablock_index*/{
        unsafe{
            level1_block.as_mut().full_mask.set_bit::<false>(in_block_level1_index);
        }
        self.insert_datablock(Block::full(), level1_block, in_block_level1_index)
    }    
    
    #[inline]
    fn try_pack_full_datablock(
        &mut self,
        level1_block_index: usize, mut level1_block: NonNull<Level1Block<Conf>>, 
        in_block_level1_index: usize,
        data_block_index: usize, data_block: NonNull<LevelDataBlock<Conf>>
    ) -> bool {
        debug_assert!(data_block_index != 1);
        let data_block = unsafe{data_block.as_ref()};
        if !data_block.is_full() {
            return false;
        }
        
        // replace data block with filled
        unsafe{
            // 1. at data level - make block empty, and remove
            Self::remove_data_block(&mut self.bitset.data, data_block_index);
            
            // 2. at level1 - change pointer to "full"
            *level1_block.as_mut()
                .block_indices.as_mut()
                .get_unchecked_mut(in_block_level1_index) = Primitive::from_usize(1);
            
            // 3. update full_mask
            level1_block.as_mut().full_mask.set_bit::<true>(in_block_level1_index);
        }
        true
    }
    
    // should be internals of fill_level1_range
    #[inline]
    fn fill_data_block<const FILL: bool>(
        &mut self, 
        level1_block_index: usize, mut level1_block: NonNull<Level1Block<Conf>>,
        in_block_level1_index: usize, 
        f: impl FnOnce(&mut [u64]))
    {
        let (data_block_index, mut data_block) =
        if FILL{
            let (data_block_index, data_block) = unsafe {
                self.bitset.get_or_insert_datablock(level1_block, in_block_level1_index)
            };
            if data_block_index == 1 {
                // block already full
                return;
            }
            (data_block_index, data_block)
        } else {
            let mut data_block_index = unsafe{
                level1_block.as_ref()
                .block_indices.as_ref()
                .get_unchecked(in_block_level1_index).as_usize()
            };
            if data_block_index == 0 {
                // block already empty
                return;
            }
            if data_block_index == 1 {
                data_block_index = self.unpack_full_datablock(level1_block, in_block_level1_index);
            }
            let data_block = unsafe{
                self.bitset.data.blocks_mut().get_unchecked_mut(data_block_index)
            };             
            (data_block_index, NonNull::from(data_block))
        };
        
        unsafe{
            f(data_block.as_mut().mask.as_array_mut());
        }
        
        if FILL{
            self.try_pack_full_datablock(
                level1_block_index, level1_block, 
                in_block_level1_index, 
                data_block_index, data_block
            );
        } else {
            self.bitset.try_pack_empty_datablock(
                in_block_level1_index, level1_block,
                data_block_index, data_block
            );
        }
    }
    
    /// # Safety
    /// 
    /// `range` must be in `level_block` range. 
    #[inline]
    unsafe fn coarse_fill_level_block<const FILL: bool, Mask, BlockIndex, BlockIndices>(
        _: BoolConst<FILL>,
        mut level_block: NonNull<Block<Mask, BlockIndex, BlockIndices>>,
        range: Range<usize>,
        mut block_remove_fn: impl FnMut(usize)
    ) where
        Mask: BitBlock,
        BlockIndex: Primitive,
        BlockIndices: AsRef<[BlockIndex]> + AsMut<[BlockIndex]> + Clone
    {
        if range.is_empty(){
            return;
        }
        debug_assert!(range.end <= Block::<Mask, BlockIndex, BlockIndices>::size());
        let range = range.start..=range.end-1;
        
        // 1. remove all non-fixed blocks.
        traverse_one_bits_array_range_unchecked(
            level_block.as_ref().mask.into_array(), range.clone(), 
            |level0_index|{
                block_remove_fn(level0_index);
                Continue(())
            }
        );
        
        let level_block = unsafe{ level_block.as_mut() };
        
        // 2. fill level0 index-pointers
        level_block
            .block_indices.as_mut()
            .get_unchecked_mut(range.clone())
            .fill(Primitive::from_usize(FILL as usize));
        
        // 3. fill mask
        fill_bits_array_unchecked::<FILL, _>(
            level_block.mask.as_array_mut(),
            range.clone()
        );
        
        // 4. fill full mask
        fill_bits_array_unchecked::<FILL, _>(
            level_block.full_mask.as_array_mut(),
            range.clone()
        );             
    }
    
    fn fill_level1_range<const FILL: bool>(
        &mut self,
        in_block_level0_index: usize,
        first_level1_index: usize, first_data_index: usize,
        last_level1_index : usize, last_data_index : usize,
    ){
        // 0. Get level1 block
        let (level1_block_index, mut level1_block) = 
        if FILL {
            let (level1_block_index, level1_block) = unsafe {
                self.bitset.get_or_insert_level1block(in_block_level0_index)
            };
            if level1_block_index == 1{
                // already full
                return;
            }            
            (level1_block_index, level1_block)
        } else {
            let mut level1_block_index = unsafe {
                self.bitset.level0.block_indices.as_mut().get_unchecked_mut(in_block_level0_index)
            }.as_usize();
            if level1_block_index == 0 {
                // already empty
                return;
            }
            if level1_block_index == 1 {
                level1_block_index = unsafe{ self.unpack_full_level1block(in_block_level0_index) };
            }
            let level1_block = unsafe{
                self.bitset.level1.blocks_mut().get_unchecked_mut(level1_block_index)
            };
            (level1_block_index, NonNull::from(level1_block))
        };
        
        let full_leftest_data  = first_data_index == 0;
        let full_rightest_data = last_data_index == LevelDataBlock::<Conf>::size() - 1;
        
        // I. Coarse fill data blocks
        unsafe{
            // let range = range_start..range_end;
            let range_start = first_level1_index + !full_leftest_data as usize;
            let range_end   = last_level1_index  + full_rightest_data as usize;
            
            Self::coarse_fill_level_block(BoolConst::<FILL>,
                level1_block, range_start..range_end,
                |level1_index|{
                    let data_block_index = unsafe {
                        level1_block.as_ref().block_indices.as_ref().get_unchecked(level1_index).as_usize()
                    };
                    
                    // remove non-fixed data block
                    if data_block_index > 1 {
                        Self::remove_data_block(&mut self.bitset.data, data_block_index);
                    }
                }
            );
        }
        
        // II. Fine fill edge data blocks.
        if first_level1_index == last_level1_index{
            self.fill_data_block::<FILL>(level1_block_index, level1_block, 
                first_level1_index, 
                |bits| unsafe{ fill_bits_array_unchecked::<FILL, _>(
                    bits, first_data_index..=last_data_index
                ) }
            );
        } else {
            self.fill_data_block::<FILL>(level1_block_index, level1_block, 
                first_level1_index, 
                |bits| unsafe{ fill_bits_array_from_unchecked::<FILL, _>(
                    bits, first_data_index..
                ) }
            );
            self.fill_data_block::<FILL>(level1_block_index, level1_block, 
                last_level1_index, 
                |bits| unsafe{ fill_bits_array_to_unchecked::<FILL, _>(
                    bits, ..=last_data_index
                ) }
            );
        }
        
        // III. Try to replace whole block with static "filled".
        if FILL{
            self.try_pack_full_level1block(in_block_level0_index, level1_block_index, level1_block);
        } else {
            self.bitset.try_pack_empty_level1block(in_block_level0_index, level1_block_index, level1_block);
        }
    }
    
    #[inline]
    fn fill_range<const FILL: bool>(&mut self, range: RangeInclusive<usize>) {
        let (first_index, last_index) = range.into_inner();
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
        
        // I. Coarse fill level1 blocks.
        unsafe{
            // let range = range_start..range_end;
            let range_start = first_level0_index + !full_leftest_level1 as usize;
            let range_end   = last_level0_index  + full_rightest_level1 as usize;
            
            let BitSet{level0, level1, data} = &mut self.bitset;
            let level0 = level0.into();
            
            Self::coarse_fill_level_block(BoolConst::<FILL>,
                level0, range_start..range_end,
                |level0_index|{
                    let level1_block_index = unsafe {
                        level0.as_ref().block_indices.as_ref().get_unchecked(level0_index).as_usize()
                    };
                    
                    // remove non-fixed level1 block
                    if level1_block_index > 1 {
                        Self::remove_level1_block(level1, data, level1_block_index);
                    }
                }                
            );
        }
        
        // II. fill edge level1 blocks
        if first_level0_index == last_level0_index{
            self.fill_level1_range::<FILL>(
                first_level0_index,
                first_level1_index, first_data_index,
                last_level1_index , last_data_index ,
            );
        } else {
            self.fill_level1_range::<FILL>(
                first_level0_index,
                first_level1_index, first_data_index,
                Level1Block::<Conf>::size() - 1, LevelDataBlock::<Conf>::size() - 1
            );
            self.fill_level1_range::<FILL>(
                last_level0_index,
                0, 0,
                last_level1_index,  last_data_index,
            );
        }
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
        self.fill_range::<true>(range);
    }
    
    /// See [insert_range].
    pub fn remove_range(&mut self, range: RangeInclusive<usize>){
        self.fill_range::<false>(range);
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
        
        if mutated_primitive == u64::MAX    // fast check for just mutated part of bitblock
        && data_block_index != 1 {   
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
        let (mut level1_block_index, level1_block, data_block_index) = self.bitset.get_block_indices(in_block_level0_index, in_block_level1_index);
        if data_block_index == 0{
            return false;
        }
        // try unpack full block
        if data_block_index == 1{
            if level1_block_index == 1{
                level1_block_index = unsafe{ self.unpack_full_level1block(in_block_level0_index) };
            }
            let level1_block = NonNull::from(unsafe{
                 self.bitset.level1.blocks_mut().get_unchecked_mut(level1_block_index)
            });
            
            let datablock_index = self.unpack_full_datablock(level1_block, in_block_level1_index);
            
            // TODO: maybe just run `remove_impl` instead?
            // we will not run remove_impl, so just directly remove bit
            let datablock = unsafe{ self.bitset.data.blocks_mut().get_unchecked_mut(datablock_index) };
            datablock.mask.set_bit::<false>(in_block_data_index);

            return true;
        }
 
        // remove as usual
        unsafe{
            self.bitset.remove_impl(
                in_block_level0_index, in_block_level1_index, in_block_data_index,
                level1_block_index, level1_block, 
                data_block_index
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
    use std::ops::RangeInclusive;
    use itertools::{assert_equal, Itertools};
    use rand::Rng;
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
    
    #[test]
    fn insert_range_regression_test(){
        let mut set: BitSetRanges<config::_64bit> = Default::default();
        set.insert_range(10008..=15059);
        set.insert_range(7626..=15769);
        assert_equal(&set, 7626..=15769);
    }
    
    #[test]
    fn range_fuzzy_test(){
        cfg_if::cfg_if! {
        if #[cfg(miri)] {
            const REPEATS: usize = 2;
            const INNER_REPEATS: usize = 3;
            const MAX_INSERTS: usize = 5;
            const MAX_REMOVES: usize = 5;
            const MAX_RANGE: usize = 20_000;
        } else {
            const REPEATS: usize = 1000;
            const INNER_REPEATS: usize = 20;
            const MAX_INSERTS: usize = 5;
            const MAX_REMOVES: usize = 5;
            const MAX_RANGE: usize = 20_000;
        }}
        
        let mut rng = rand::thread_rng();
        let mut gen_range = {
            let mut rng = rng.clone();
            move ||{
                let start = rng.gen_range(0..MAX_RANGE);
                let end   = rng.gen_range(start..MAX_RANGE);
                start..end
            }
        };
        
        for _ in 0..REPEATS{
            //println!("------");
            let mut vec: Vec<bool> = vec![false; MAX_RANGE];
            let mut set: BitSetRanges<config::_64bit> = Default::default();
            
            for _ in 0..INNER_REPEATS {
                // random insert
                for _ in 0..rng.gen_range(0..MAX_INSERTS){
                    let range = gen_range();
                    if !range.is_empty(){
                        let range = range.start..=range.end-1;
                        set.insert_range(range.clone());
                        vec[range.clone()].fill(true);
                        //println!("insert {:?}", range.clone());
                    }
                }
                
                // random remove
                for _ in 0..rng.gen_range(0..MAX_REMOVES){
                    let range = gen_range();
                    if !range.is_empty(){
                        let range = range.start..=range.end-1;
                        set.remove_range(range.clone());
                        vec[range].fill(false);
                    }
                }
                
                let vec_elements = 
                    vec.iter().enumerate()
                    .filter_map(|(index, b)|{
                        if *b{Some(index)} else {None}
                    });
                assert_equal(&set, vec_elements);
            }
        }
    }
    
    #[test]
    fn range_remove_test(){
        let fill_range = 0..20_000;
        let bitset: BitSetRanges<config::_64bit> = fill_range.clone().collect();
        assert_equal(&bitset, fill_range.clone());
        
        let check = |range: RangeInclusive<usize>|{
            let mut bitset = bitset.clone();
            bitset.remove_range(range.clone());
            let (start, end) = range.into_inner();
            assert_equal(&bitset, (0..start).chain(end+1..fill_range.end));
        };
        
        // left+coarse+right
        check(34..=4096*2+18);
        
        // left+coarse
        check(34..=4096*2-1);
        
        // right+coarse
        check(0..=4096*2+38);
        
        // coarse
        check(4096..=4096*2-1);
        
        // no level0 coarse
        check(34..=751);
    }
    
    #[test]
    fn range_insert_test(){
        // left+coarse+right
        {
            let range = 34..=4096*2+18;
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
        
        // right+coarse
        {
            let range = 0..=4096*2+38;
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
        
        // no level0 coarse
        {
            let range = 34..=751;
            let mut bitset: BitSetRanges<config::_64bit> = Default::default();
            bitset.insert_range(range.clone());
            assert_equal(&bitset, range.clone());
        }
    }
}