use std::mem::{ManuallyDrop, MaybeUninit};
use crate::{assume, BitSetBase, internals};
use crate::bitset_interface::{LevelMasks, LevelMasksIterExt};
use crate::config::{Config};
use crate::block::Block;
use crate::raw;

type Level0Block<Conf> = Block<
    <Conf as Config>::Level0BitBlock, 
    <Conf as Config>::Level0BlockIndices
>;
type Level1Block<Conf> = Block<
    <Conf as Config>::Level1BitBlock,
    <Conf as Config>::Level1BlockIndices
>;
type LevelDataBlock<Conf> = Block<
    <Conf as Config>::DataBitBlock, [usize;0]
>;

type RawBitSet<Conf> = raw::RawBitSet<
    Conf,
    Level0Block<Conf>,
    Level1Block<Conf>,
    LevelDataBlock<Conf>
>;

/// Hierarchical sparse bitset.
///
/// Tri-level hierarchy. Highest uint it can hold
/// is [Level0BitBlock]::size() * [Level1BitBlock]::size() * [DataBitBlock]::size().
///
/// Only last level contains blocks of actual data. Empty(skipped) data blocks
/// are not allocated.
///
/// Structure optimized for intersection speed. 
/// _(Other inter-bitset operations are in fact fast too - but intersection has lowest algorithmic complexity.)_
/// Insert/remove/contains is fast O(1) too.
/// 
/// [Level0BitBlock]: crate::config::Config::Level0BitBlock
/// [Level1BitBlock]: crate::config::Config::Level1BitBlock
/// [DataBitBlock]: crate::config::Config::DataBitBlock
pub struct BitSet<Conf: Config>(
    RawBitSet<Conf>
);

impl<Conf: Config> BitSet<Conf> {
    #[inline]
    pub fn new() -> Self{
        Default::default()
    }    
    
    /// # Safety
    ///
    /// Will panic, if `index` is out of range.    
    #[inline]
    pub fn insert(&mut self, index: usize){
        self.0.insert(index)
    }
    
    /// Returns false if index is invalid/not in bitset.
    #[inline]
    pub fn remove(&mut self, index: usize) -> bool {
        self.0.remove(index)
    }
    
    /// # Safety
    ///
    /// `index` MUST exists in HiSparseBitset!
    #[inline]
    pub unsafe fn remove_unchecked(&mut self, index: usize) {
        // TODO: make sure compiler actually get rid of unused code.
        let ok = self.remove(index);
        unsafe{ assume!(ok); }
    }    
}

impl<Conf: Config> Clone for BitSet<Conf> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Conf: Config> Default for BitSet<Conf> {
    #[inline]
    fn default() -> Self{
        Self(Default::default())
    }
}

impl<Conf: Config> FromIterator<usize> for BitSet<Conf> {
    #[inline]
    fn from_iter<T: IntoIterator<Item=usize>>(iter: T) -> Self {
        Self(RawBitSet::<Conf>::from_iter(iter))
    }
}

impl<Conf: Config, const N: usize> From<[usize; N]> for BitSet<Conf> {
    #[inline]
    fn from(value: [usize; N]) -> Self {
        Self(RawBitSet::<Conf>::from(value))
    }
}

impl<Conf: Config> BitSetBase for BitSet<Conf>{
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = RawBitSet::<Conf>::TRUSTED_HIERARCHY;
}

impl<Conf: Config> LevelMasks for BitSet<Conf>{
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

impl<Conf: Config> LevelMasksIterExt for BitSet<Conf>{
    type IterState = <RawBitSet<Conf> as LevelMasksIterExt>::IterState;
    type Level1BlockData = <RawBitSet<Conf> as LevelMasksIterExt>::Level1BlockData;
    
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
        RawBitSet::<Conf>::data_mask_from_block_data(level1_block_data, level1_index)
    }
}

internals::impl_bitset!(impl<Conf> for ref BitSet<Conf> where Conf: Config);