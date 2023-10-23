use std::marker::PhantomData;
use std::ops;
use num_traits::AsPrimitive;
use crate::bit_block::BitBlock;
use crate::{DataBlock, HiSparseBitset, IConfig, LevelMasks};

struct State<Config: IConfig> {
    level0_iter: <Config::Level0BitBlock as BitBlock>::BitsIter,
    level1_iter: <Config::Level1BitBlock as BitBlock>::BitsIter,
    level0_index: usize,
}

// Op = BitAnd

pub struct Reduce<'a, Config, SetLike, S>
where
    Config: IConfig,
    SetLike: LevelMasks<Config> + 'a,
    S: Iterator<Item = &'a SetLike> + Clone
{
    sets: S,
    //state: State<Config>,
    phantom: PhantomData<Config>
}

impl<'a, Config, SetLike, S> LevelMasks<Config> for Reduce<'a, Config, SetLike, S>
where
    Config: IConfig,
    SetLike: LevelMasks<Config> + 'a,
    S: Iterator<Item = &'a SetLike> + Clone,
{
    /// Will computate.
    fn level0_mask(&self) -> Config::Level0BitBlock {
        self.sets.clone()
        .map(|set| set.level0_mask())
        .reduce(ops::BitAnd::bitand)
        .unwrap()
    }

    unsafe fn level1_mask(&self, level0_index: usize) -> Config::Level1BitBlock {
        unsafe{
            self.sets.clone()
            .map(|set| {
                set.level1_mask(level0_index)
            })
            .reduce(ops::BitAnd::bitand)
            .unwrap_unchecked()
        }
    }

    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize) -> Config::DataBitBlock {
        unsafe{
            self.sets.clone()
            .map(|set| {
                set.data_mask(level0_index, level1_index)
            })
            .reduce(ops::BitAnd::bitand)
            .unwrap_unchecked()
        }
    }
}


pub struct ReduceIter<'a, Config, SetLike, S>
where
    Config: IConfig,
    SetLike: LevelMasks<Config> + 'a,
    S: Iterator<Item = &'a SetLike> + Clone
{
    reduce: Reduce<'a, Config, SetLike, S>,
    state: State<Config>,
    //phantom: PhantomData<Op>
}


impl<'a, Config, SetLike, S> Iterator for ReduceIter<'a, Config, SetLike, S>
where
    Config: IConfig,
    SetLike: LevelMasks<Config> + 'a,
    S: Iterator<Item = &'a SetLike> + Clone
{
    type Item = DataBlock<Config::DataBitBlock>;

    fn next(&mut self) -> Option<Self::Item> {
        let Self{reduce, state} = self;

        let level1_index =
            loop{
                if let Some(index) = state.level1_iter.next(){
                    break index;
                } else {
                    //update level0
                    if let Some(index) = state.level0_iter.next(){
                        state.level0_index = index;

                        // update level1 iter
                        let level1_intersection = unsafe {
                            reduce.level1_mask(index.as_())
                        };
                        state.level1_iter = level1_intersection.bits_iter();
                    } else {
                        return None;
                    }
                }
            };

        let data_intersection = unsafe {
             self.reduce.data_mask(state.level0_index, level1_index)
        };

        let block_start_index = (state.level0_index << (Config::DataBitBlock::SIZE_POT_EXPONENT + Config::Level1BitBlock::SIZE_POT_EXPONENT))
            + (level1_index << (Config::DataBitBlock::SIZE_POT_EXPONENT));

        Some(DataBlock{ start_index: block_start_index, bit_block: data_intersection })
    }
}
