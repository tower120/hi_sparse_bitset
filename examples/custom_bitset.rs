//! This example shows how to make custom bitset, leveraging LevelMasksIterExt
//! to achieve maximum performance. See [examples/custom_bitset_simple] for 
//! simpler version.
//! 
//! Requires `impl` feature to build.

use std::marker::PhantomData;
use std::mem::{ManuallyDrop, MaybeUninit};
use hi_sparse_bitset::config::Config;
use hi_sparse_bitset::{BitBlock, BitSetBase, impl_bitset};
use hi_sparse_bitset::internals::{LevelMasks, LevelMasksIterExt};

#[derive(Default)]
struct Empty<Conf: Config>(PhantomData<Conf>);

impl<Conf: Config> BitSetBase for Empty<Conf> {
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = true; 
}

impl<Conf: Config> LevelMasks for Empty<Conf> {
    fn level0_mask(&self) -> <Self::Conf as Config>::Level0BitBlock {
        BitBlock::zero()
    }

    unsafe fn level1_mask(&self, _level0_index: usize)
        -> <Self::Conf as Config>::Level1BitBlock 
    { 
        BitBlock::zero()
    }

    unsafe fn data_mask(&self, _level0_index: usize, _level1_index: usize)
        -> <Self::Conf as Config>::DataBitBlock
    {
        BitBlock::zero()
    }
}

impl<Conf: Config> LevelMasksIterExt for Empty<Conf> {
    type IterState = ();
    type Level1BlockData = ();

    fn make_iter_state(&self) -> Self::IterState { () }
    unsafe fn drop_iter_state(&self, _state: &mut ManuallyDrop<Self::IterState>) {}

    unsafe fn init_level1_block_data(
        &self, 
        _state: &mut Self::IterState, 
        _level1_block_data: &mut MaybeUninit<Self::Level1BlockData>, 
        _level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        (BitBlock::zero(), false)
    }
    
    unsafe fn data_mask_from_block_data(
        _level1_block_data: &Self::Level1BlockData, _level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock {
        BitBlock::zero()
    }
}

impl_bitset!(
    impl<Conf> for Empty<Conf> where Conf: Config
);

fn main(){
    type Conf = hi_sparse_bitset::config::_64bit;
    let empty = Empty::<Conf>::default();
    assert!(empty.is_empty());
    assert!(!empty.contains(10));
    assert!(empty.iter().next().is_none());
}