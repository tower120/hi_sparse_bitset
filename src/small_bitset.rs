use std::mem::{ManuallyDrop, MaybeUninit};
use crate::block::Block;
use crate::compact_block::CompactBlock;
use crate::config::{Config, ConfigSmall};
use crate::{BitSetBase, internals};
use crate::bitset_interface::{LevelMasks, LevelMasksIterExt};
use crate::raw::RawBitSet;

type Level0Block<Conf> = Block<
    <Conf as Config>::Level0BitBlock, 
    <Conf as Config>::Level0BlockIndices
>;
type Level1Block<Conf> = CompactBlock<
    <Conf as Config>::Level1BitBlock,
    <Conf as ConfigSmall>::Level1MaskU64Populations,
    <Conf as Config>::Level1BlockIndices,
    <Conf as ConfigSmall>::Level1SmallBlockIndices,
>;
type LevelDataBlock<Conf> = Block<
    <Conf as Config>::DataBitBlock, [usize;0]
>;

type RawSmallBitSet<Conf> = RawBitSet<
    Conf,
    Level0Block<Conf>,
    Level1Block<Conf>,
    LevelDataBlock<Conf>
>; 

pub struct SmallBitSet<Conf: ConfigSmall>(
    RawBitSet<
        Conf,
        Level0Block<Conf>,
        Level1Block<Conf>,
        LevelDataBlock<Conf>
    >
);

impl<Conf: ConfigSmall> Clone for SmallBitSet<Conf> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Conf: ConfigSmall> Default for SmallBitSet<Conf> {
    #[inline]
    fn default() -> Self{
        Self(Default::default())
    }
}

impl<Conf: ConfigSmall> FromIterator<usize> for SmallBitSet<Conf> {
    #[inline]
    fn from_iter<T: IntoIterator<Item=usize>>(iter: T) -> Self {
        Self(RawSmallBitSet::<Conf>::from_iter(iter))
    }
}

impl<Conf: ConfigSmall, const N: usize> From<[usize; N]> for SmallBitSet<Conf> {
    #[inline]
    fn from(value: [usize; N]) -> Self {
        Self(RawSmallBitSet::<Conf>::from(value))
    }
}

impl<Conf: ConfigSmall> SmallBitSet<Conf> {
    #[inline]
    pub fn new() -> Self{
        Default::default()
    }    
    
    #[inline]
    pub fn insert(&mut self, index: usize){
        self.0.insert(index)
    }
    
    #[inline]
    pub fn remove(&mut self, index: usize) -> bool {
        self.0.remove(index)
    }
}

impl<Conf: ConfigSmall> BitSetBase for SmallBitSet<Conf>{
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = RawSmallBitSet::<Conf>::TRUSTED_HIERARCHY;
}

impl<Conf: ConfigSmall> LevelMasks for SmallBitSet<Conf>{
    #[inline]
    fn level0_mask(&self) -> <Self::Conf as Config>::Level0BitBlock {
        self.0.level0_mask()
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize) -> <Self::Conf as Config>::Level1BitBlock {
        self.0.level1_mask(level0_index)
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize) -> <Self::Conf as Config>::DataBitBlock {
        self.0.data_mask(level0_index, level1_index)
    }
}

impl<Conf: ConfigSmall> LevelMasksIterExt for SmallBitSet<Conf>{
    type IterState = <RawSmallBitSet<Conf> as LevelMasksIterExt>::IterState;
    type Level1BlockData = <RawSmallBitSet<Conf> as LevelMasksIterExt>::Level1BlockData;

    #[inline]
    fn make_iter_state(&self) -> Self::IterState {
        self.0.make_iter_state()
    }

    #[inline]
    unsafe fn drop_iter_state(&self, state: &mut ManuallyDrop<Self::IterState>) {
        self.0.drop_iter_state(state)
    }

    #[inline]
    unsafe fn init_level1_block_data(&self, state: &mut Self::IterState, level1_block_data: &mut MaybeUninit<Self::Level1BlockData>, level0_index: usize) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        self.0.init_level1_block_data(state, level1_block_data, level0_index)
    }

    #[inline]
    unsafe fn data_mask_from_block_data(level1_block_data: &Self::Level1BlockData, level1_index: usize) -> <Self::Conf as Config>::DataBitBlock {
        RawSmallBitSet::<Conf>::data_mask_from_block_data(level1_block_data, level1_index)
    }
}

internals::impl_bitset!(impl<Conf> for ref SmallBitSet<Conf> where Conf: ConfigSmall);