use num_traits::AsPrimitive;
use crate::{data_block_start_index, DataBlock, IConfig};
use crate::bit_block::BitBlock;
use crate::bit_queue::BitQueue;
use crate::virtual_bitset::{LevelMasks, LevelMasksExt3};

// TODO: Clone -able.
/// Iterator state. Acts like cursor, or position of iterable.
///
/// Allows to resume iteration from the last position, even if
/// source was mutated. Both suspending and resuming operations are very fast.
///
/// Can be used with ALL virtual sets.
/// Default constructed State will traverse virtual set from the very begin.
///
/// # Resume
///
/// After resume from State, iterator will continue iteration from where
/// it was suspended. All elements that was removed since suspension will
/// not appear in iteration sequence. Newly added elements may sporadically appear
/// in output.
///
/// IOW - you're guaranteed to have your initial sequence in valid state +
/// some new valid elements (if any was added).
///
/// ## Resume from index (?)
///
/// TODO
///
/// Iterator will be resumed from last processed block and will go forward.
/// All blocks BEFORE start position will not be iterated, all blocks AFTER will.
///
/// _IOW - resumed iterator will behave like a new one, but with 0..index blocks discarded._
///
/// # Use-case
///
/// This can be used to split long iteration into a few sessions.
/// You may want that in concurrent environment, when you can't process whole
/// iteration sequence fast, and want not to keep lock
/// on resource all the time you process iteration sequence.
///
/// Example: you lock sets, make intersection iterator, read 40 blocks into
/// buffer, suspend iterator to state, unlock sets, process buffer, lock sets,
/// resume iterator from state, and so on.
///
pub struct State<Config: IConfig> {
    pub(crate) level0_iter: <Config::Level0BitBlock as BitBlock>::BitsIter,
    pub(crate) level1_iter: <Config::Level1BitBlock as BitBlock>::BitsIter,
    pub(crate) level0_index: usize,
}
impl<Config: IConfig> Default for State<Config>{
    /// Iteration will start from the very begin.
    ///
    /// It is safe to use any virtual sets with default constructed `State`.
    #[inline]
    fn default() -> Self {
        Self {
            level0_iter: BitQueue::filled(),
            level1_iter: BitQueue::empty(),
            level0_index: 0
        }
    }

    // TODO: consider returning "resume()" here
}

/// Remove non-existent elements from `state` internal iterators.
fn patch_state<T, Config, F>(
    state: &mut State<Config>, virtual_set: &T, mut level1_mask_gen: F
) where
    T: LevelMasks<Config = Config>,
    Config: IConfig,
    F: FnMut(usize) -> Config::Level1BitBlock
{
    // Level0
    let level0_mask = virtual_set.level0_mask();
    let level0_index_valid = level0_mask.get_bit(state.level0_index);
    state.level0_iter.mask_out(level0_mask.as_array_u64());

    // Level1
    if level0_index_valid {
        let level1_mask = level1_mask_gen(state.level0_index);
        state.level1_iter.mask_out(level1_mask.as_array_u64());
    } else {
        // Don't touch `level0_index`.
        // It will be updated in iterator.
        state.level1_iter  = BitQueue::empty();
    }
}

/// Simple iterator - access each data block, by traversing all hierarchy
/// levels indirections each time.
///
/// Does not cache intermediate level1 position - hence have MUCH smaller size.
/// May have similar to [Iter] performance on very sparse sets.
///
/// # Motivation
///
/// The only reason why you might want to use this - is size.
/// `SimpleIter` according to benchmarks can be up to x2 slower,
/// but usually difference around x1.5.
pub struct SimpleIter<T>
where
    T: LevelMasks,
{
    virtual_set: T,
    state: State<T::Config>,
}

impl<T> SimpleIter<T>
where
    T: LevelMasks
{
    #[inline]
    pub fn new(virtual_set: T) -> Self {
        let state = State{
            level0_iter: virtual_set.level0_mask().bits_iter(),
            level1_iter: BitQueue::empty(),
            level0_index: 0,
        };
        Self{
            virtual_set,
            state,
        }
    }

    pub fn resume(virtual_set: T, mut state: State<T::Config>) -> Self {
        let lvl1_mask_gen = |index| unsafe {
            virtual_set.level1_mask(index)
        };
        patch_state(&mut state, &virtual_set, lvl1_mask_gen);
        Self{
            virtual_set,
            state,
        }
    }

    #[inline]
    pub fn suspend(self) -> State<T::Config> {
        self.state
    }
}


impl<T> Iterator for SimpleIter<T>
where
    T: LevelMasks,
{
    type Item = DataBlock<<<T as LevelMasks>::Config as IConfig>::DataBitBlock>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self{ virtual_set, state} = self;

        let level1_index =
            loop{
                if let Some(index) = state.level1_iter.next(){
                    break index;
                } else {
                    //update level0
                    if let Some(index) = state.level0_iter.next(){
                        state.level0_index = index;

                        // update level1 iter
                        let level1_mask = unsafe {
                            virtual_set.level1_mask(index.as_())
                        };
                        state.level1_iter = level1_mask.bits_iter();
                    } else {
                        return None;
                    }
                }
            };

        let data_mask = unsafe {
            self.virtual_set.data_mask(state.level0_index, level1_index)
        };

        let block_start_index =
            data_block_start_index::<<T as LevelMasks>::Config>(
                state.level0_index, level1_index
            );

        Some(DataBlock{ start_index: block_start_index, bit_block: data_mask })
    }
}


/// Fast on all operations.
///
/// Cache level1 block pointers, making data blocks access faster.
///
/// Also, can discard (on branch level) sets with empty level1 blocks from iteration.
/// (See [binary_op] - this have no effect for AND operation, but can speed up all other)
///
/// N.B. Do not move or clone without need - heavyweight due to cache.
pub struct IterExt3<T>
where
    T: LevelMasksExt3,
{
    virtual_set: T,
    state: State<T::Config>,
    level1_blocks: T::Level1Blocks3,
}

impl<T> IterExt3<T>
where
    T: LevelMasksExt3,
{
    #[inline]
    pub fn new(virtual_set: T) -> Self {
        let state = State{
            level0_iter: virtual_set.level0_mask().bits_iter(),
            level1_iter: BitQueue::empty(),
            level0_index: 0,
        };
        let level1_blocks = virtual_set.make_level1_blocks3();
        Self{
            virtual_set,
            state,
            level1_blocks
        }
    }

    pub fn resume(virtual_set: T, mut state: State<T::Config>) -> Self {
        let mut level1_blocks = virtual_set.make_level1_blocks3();
        let lvl1_mask_gen = |index| unsafe {
            // Generate both mask and level1_blocks cache
            let (mask, valid) = virtual_set.always_update_level1_blocks3(&mut level1_blocks, index);
            if !valid {
                // level1_mask can not be empty here
                std::hint::unreachable_unchecked()
            }
            mask
        };
        patch_state(&mut state, &virtual_set, lvl1_mask_gen);

        Self{
            virtual_set,
            state,
            level1_blocks
        }
    }

    #[inline]
    pub fn suspend(self) -> State<T::Config> {
        self.state
    }
}


impl<T> Iterator for IterExt3<T>
where
    T: LevelMasksExt3,
{
    type Item = DataBlock<<T::Config as IConfig>::DataBitBlock>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self{virtual_set, state, level1_blocks, ..} = self;

        let level1_index =
            loop{
                if let Some(index) = state.level1_iter.next(){
                    break index;
                } else {
                    //update level0
                    if let Some(index) = state.level0_iter.next(){
                        state.level0_index = index;

                        let (level1_intersection, valid) = unsafe {
                            virtual_set.always_update_level1_blocks3(level1_blocks, index)
                        };
                        if !valid {
                            // level1_mask can not be empty here
                            unsafe { std::hint::unreachable_unchecked() }
                        }
                        state.level1_iter = level1_intersection.bits_iter();
                    } else {
                        return None;
                    }
                }
            };

        let data_intersection = unsafe {
            T::data_mask_from_blocks3(level1_blocks, level1_index)
        };

        let block_start_index =
            data_block_start_index::<<T as LevelMasks>::Config>(
                state.level0_index, level1_index
            );

        Some(DataBlock{ start_index: block_start_index, bit_block: data_intersection })
    }
}
