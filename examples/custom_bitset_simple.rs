//! This example shows how to make custom bitset in simple form.
//! 
//! Requires `impl` feature to build.

use std::marker::PhantomData;
use std::mem::{ManuallyDrop, MaybeUninit};
use hi_sparse_bitset::config::Config;
use hi_sparse_bitset::{BitBlock, BitSetBase, impl_bitset_simple};
use hi_sparse_bitset::internals::LevelMasks;

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

impl_bitset_simple!(
    impl<Conf> for ref Empty<Conf> where Conf: Config
);

fn main(){
    type Conf = hi_sparse_bitset::config::_64bit;
    let empty = Empty::<Conf>::default();
    assert!(empty.is_empty());
    assert!(!empty.contains(10));
    assert!(empty.iter().next().is_none());
}