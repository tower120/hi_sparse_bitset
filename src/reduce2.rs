use std::marker::PhantomData;
use std::ops;
use arrayvec::ArrayVec;
use num_traits::AsPrimitive;
use crate::bit_block::BitBlock;
use crate::{data_block_start_index, DataBlock, HiSparseBitset, IConfig, LevelMasks, LevelMasksExt};
use crate::binary_op::BinaryOp;
use crate::bit_queue::BitQueue;

const MAX_SETS: usize = 32;

struct State<Config: IConfig> {
    level0_iter: <Config::Level0BitBlock as BitBlock>::BitsIter,
    level1_iter: <Config::Level1BitBlock as BitBlock>::BitsIter,
    level0_index: usize,
}

// TODO : try to remove Config from Reduce by making it type in LevelMasks trait

pub struct Reduce<Op, SetLike, S>
where
    Op: BinaryOp,
    SetLike: LevelMasks,
    S: Iterator<Item = SetLike> + Clone
{
    pub(crate) sets: S,
    pub(crate) phantom: PhantomData<(Op)>
}

impl<Op, SetLike, S> Reduce<Op, SetLike, S>
where
    Op: BinaryOp,
    SetLike: LevelMasks,
    S: Iterator<Item = SetLike> + Clone
{
    // TODO: This is BLOCK iterator. Make separate iterator for usizes.
    // TODO: Benchmark if there is need for "traverse".
    // TODO: !! Iterator must use &sets, since we store pointers to level1 inside !!
    #[inline]
    pub fn iter(self) -> ReduceIter<Op, SetLike, S> {
        let level0_iter = self.level0_mask().bits_iter();

        ReduceIter{
            reduce: self,
            state: State{
                level0_iter,
                level1_iter: BitQueue::empty(),
                level0_index: 0,
            }
        }
    }

    #[inline]
    pub fn iter_ext(self) -> ReduceIterExt<Op, SetLike, S>
    where
        SetLike: LevelMasksExt,
        S: ExactSizeIterator
    {
        let level0_iter = self.level0_mask().bits_iter();
        let level1_blocks = self.make_level1_blocks();

        ReduceIterExt{
            reduce: self,
            state: State{
                level0_iter,
                level1_iter: BitQueue::empty(),
                level0_index: 0,
            },
            level1_blocks
        }
    }
}


impl<Op, SetLike, S> LevelMasks for Reduce<Op, SetLike, S>
where
    Op: BinaryOp,
    SetLike: LevelMasks,
    S: Iterator<Item = SetLike> + Clone
{
    type Config = SetLike::Config;

    /// Will computate.
    #[inline]
    fn level0_mask(&self) -> <Self::Config as IConfig>::Level0BitBlock {
        self.sets.clone()
        .map(|set| set.level0_mask())
        .reduce(Op::op)
        .unwrap()
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize)
        -> <Self::Config as IConfig>::Level1BitBlock
    {
        unsafe{
            self.sets.clone()
            .map(|set| {
                set.level1_mask(level0_index)
            })
            .reduce(Op::op)
            .unwrap_unchecked()
        }
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize)
        -> <Self::Config as IConfig>::DataBitBlock
    {
        unsafe{
            self.sets.clone()
            .map(|set| {
                set.data_mask(level0_index, level1_index)
            })
            .reduce(Op::op)
            .unwrap_unchecked()
        }
    }
}

impl<Op, SetLike, S> LevelMasksExt for Reduce<Op, SetLike, S>
where
    Op: BinaryOp,
    SetLike: LevelMasksExt,
    S: Iterator<Item = SetLike> + Clone,
    S: ExactSizeIterator
{
    //type Level1Blocks = Vec<SetLike::Level1Blocks>;
    type Level1Blocks = ArrayVec<SetLike::Level1Blocks, MAX_SETS>;

    #[inline]
    fn make_level1_blocks(&self) -> Self::Level1Blocks {
        unsafe {
            /*let mut array = Vec::with_capacity(sets_len);
            array.set_len(sets_len);
            array*/

            let mut array = ArrayVec::new();

            // calling constructors in deep
            for (index, set) in self.sets.clone().enumerate(){
                std::ptr::write(
                    array.get_unchecked_mut(index),
                    set.make_level1_blocks()
                );
            }

            array.set_len(self.sets.len());
            array
        }
    }

    #[inline]
    unsafe fn update_level1_blocks(
        &self, level1_blocks: &mut Self::Level1Blocks, level0_index: usize
    ) {
        for (index, set) in self.sets.clone().enumerate(){
            set.update_level1_blocks(level1_blocks.get_unchecked_mut(index), level0_index);
        }
    }

    #[inline]
    unsafe fn data_mask_from_blocks(
        &self, level1_blocks: &Self::Level1Blocks, level1_index: usize
    ) -> <Self::Config as IConfig>::DataBitBlock {
        // TODO: assert same self.sets.len() == level1_blocks.len()
        let mut level1_blocks_iter = level1_blocks.into_iter();
        unsafe{
            self.sets.clone()
            .map(move |set| {
                let set_level1_blocks = level1_blocks_iter.next().unwrap_unchecked();
                set.data_mask_from_blocks(set_level1_blocks, level1_index)
            })
            .reduce(Op::op)
            .unwrap_unchecked()
        }
    }
}


pub struct ReduceIter<Op, SetLike, S>
where
    Op: BinaryOp,
    SetLike: LevelMasks,
    S: Iterator<Item = SetLike> + Clone
{
    reduce: Reduce<Op, SetLike, S>,
    state: State<SetLike::Config>,
    //phantom: PhantomData<Op>
}


impl<Op, SetLike, S> Iterator for ReduceIter<Op, SetLike, S>
where
    Op: BinaryOp,
    SetLike: LevelMasks,
    S: Iterator<Item = SetLike> + Clone
{
    type Item = DataBlock<<SetLike::Config as IConfig>::DataBitBlock>;

    #[inline]
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

        let block_start_index = data_block_start_index::<SetLike::Config>(
            state.level0_index, level1_index
        );

        Some(DataBlock{ start_index: block_start_index, bit_block: data_intersection })
    }
}



pub struct ReduceIterExt<Op, SetLike, S>
where
    Op: BinaryOp,
    SetLike: LevelMasksExt,
    S: Iterator<Item = SetLike> + Clone,
    S: ExactSizeIterator
{
    reduce: Reduce<Op, SetLike, S>,
    state: State<SetLike::Config>,
    //phantom: PhantomData<Op>

    level1_blocks: <Reduce<Op, SetLike, S> as LevelMasksExt>::Level1Blocks
}

impl<Op, SetLike, S> Iterator for ReduceIterExt<Op, SetLike, S>
where
    Op: BinaryOp,
    SetLike: LevelMasksExt,
    S: Iterator<Item = SetLike> + Clone,
    S: ExactSizeIterator
{
    type Item = DataBlock<<SetLike::Config as IConfig>::DataBitBlock>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self{reduce, state, level1_blocks} = self;

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

                        // update level1_blocks from sets
                        unsafe {
                            reduce.update_level1_blocks(level1_blocks, state.level0_index);
                        }
                    } else {
                        return None;
                    }
                }
            };

        let data_intersection = unsafe {
            self.reduce.data_mask_from_blocks(level1_blocks, level1_index)
        };

        let block_start_index = data_block_start_index::<SetLike::Config>(
            state.level0_index, level1_index
        );

        Some(DataBlock{ start_index: block_start_index, bit_block: data_intersection })
    }
}
