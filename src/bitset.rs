mod block;
mod level;
pub(crate) mod serialization;
#[cfg(feature="serde")]
mod serde;

mod mem_info;
pub use mem_info::*;

use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign};
use std::ptr::NonNull;
use crate::config::Config;
use crate::ops::BitSetOp;
use block::Block;
use crate::bitset::level::{IBlock, Level};
use crate::{BitBlock, BitSetBase, BitSetInterface, DataBlock, level_indices};
use crate::bitset_interface::{LevelMasks, LevelMasksIterExt};
use crate::internals::{impl_bitset, Primitive};

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

/// Hierarchical sparse bitset.
///
/// Tri-level hierarchy. Highest uint it can hold
/// is [Level0BitBlock]::size() * [Level1BitBlock]::size() * [DataBitBlock]::size().
///
/// Only the last level contains blocks of actual data. Empty(skipped) data blocks
/// are not allocated.
///
/// Structure optimized for intersection speed.
/// _(Other inter-bitset operations are in fact fast too - but intersection
/// has the lowest algorithmic complexity.)_
/// Insert/remove/contains is fast O(1) too.
///
/// [Level0BitBlock]: crate::config::Config::Level0BitBlock
/// [Level1BitBlock]: crate::config::Config::Level1BitBlock
/// [DataBitBlock]: crate::config::Config::DataBitBlock
pub struct BitSet<Conf: Config> {
    level0: Level0Block<Conf>,
    level1: Level<Level1Block<Conf>>,
    data  : Level<LevelDataBlock<Conf>>,
}

impl<Conf: Config> Clone for BitSet<Conf> {
    fn clone(&self) -> Self {
        Self{
            level0: self.level0.clone(),
            level1: self.level1.clone(),
            data  : self.data.clone(),
        }
    }
}

impl<Conf: Config> Default for BitSet<Conf> {
    fn default() -> Self {
        Self{
            level0: Default::default(),
            level1: Default::default(),
            data  : Default::default(),
        }
    }
}

impl<Conf: Config> FromIterator<usize> for BitSet<Conf> {
    fn from_iter<T: IntoIterator<Item=usize>>(iter: T) -> Self {
        let mut this = Self::default();
        this.extend(iter);
        this
    }
}

impl<Conf: Config> FromIterator<DataBlock<Conf::DataBitBlock>> for BitSet<Conf> {
    /// It is allowed for blocks with the same range to repeat in iterator.
    /// Like `([1,42], [15,27,61])`. Their data will be merged.
    fn from_iter<T: IntoIterator<Item=DataBlock<Conf::DataBitBlock>>>(iter: T) -> Self {
        let mut this = Self::default();
        this.extend(iter);
        this
    }
}

impl<Conf: Config> Extend<usize> for BitSet<Conf> {
    fn extend<T: IntoIterator<Item=usize>>(&mut self, iter: T) {
        for i in iter {
            self.insert(i);
        }
    }
}

impl<Conf: Config> Extend<DataBlock<Conf::DataBitBlock>> for BitSet<Conf> {
    /// It is allowed for blocks with the same range to repeat in iterator.
    /// Like `([1,42], [15,27,61])`. Their data will be merged.
    fn extend<T: IntoIterator<Item=DataBlock<Conf::DataBitBlock>>>(&mut self, iter: T) {
        for b in iter {
            self.insert_block(b);
        }
    }
}

impl<Conf: Config, const N: usize> From<[usize; N]> for BitSet<Conf> {
    #[inline]
    fn from(value: [usize; N]) -> Self {
        Self::from_iter(value.into_iter())
    }
}

impl<Conf, B> From<B> for BitSet<Conf>
where
    Conf: Config,
    B: BitSetInterface<Conf = Conf>
{
    /// Materialize any [BitSetInterface].
    #[inline]
    fn from(bitset: B) -> Self {
        let mut this = Self::default();
        this.unite_impl::<B, true>(bitset);
        this
    }
}

impl<Conf: Config> BitSet<Conf> {
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
        -> &mut LevelDataBlock<Conf>
    {
        // 1. Level0
        let level1_block_index =
            self.level0.get_or_insert(level0_index, ||{
                let block_index = self.level1.insert_empty_block();
                Primitive::from_usize(block_index)
            }).as_usize();

        // 2. Level1
        let data_block_index = {
            let level1_block = self.level1.blocks_mut().get_unchecked_mut(level1_block_index);
            level1_block.get_or_insert(level1_index, ||{
                let block_index = self.data.insert_empty_block();
                Primitive::from_usize(block_index)
            }).as_usize()
        };

        // 3. Data block
        self.data.blocks_mut().get_unchecked_mut(data_block_index)
    }

    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    /// Memory usage info.
    #[inline]
    pub fn mem_info(&self) -> MemInfo<'_, Conf> {
        MemInfo { bitset: self }
    }

    /// # Panics
    ///
    /// Panics, if `index` is out of index range.
    pub fn insert(&mut self, index: usize){
        assert!(Self::is_in_range(index), "{index} is out of index range!");

        // That's indices to next level
        let (level0_index, level1_index, data_index) = Self::level_indices(index);

        unsafe{
            let data_block = self.get_or_insert_data_block(level0_index, level1_index);
            data_block.mask_mut().set_bit_unchecked::<true>(data_index);
        }
    }

    /// # Panics
    ///
    /// Panics, if `block` is out of index range.
    pub fn insert_block(&mut self, block: DataBlock<Conf::DataBitBlock>) {
        if block.is_empty() {
            return;
        }

        assert!(
            Self::is_in_range(block.start_index + Conf::DataBitBlock::size()),
            "{:?} is out of index range!", block
        );

        // That's indices to next level
        let (level0_index, level1_index, _) = Self::level_indices(block.start_index);

        unsafe{
            let data_block = self.get_or_insert_data_block(level0_index, level1_index);
            *data_block.mask_mut() |= block.bit_block;
        }
    }

    /// Returns false if `index` is not in bitset.
    ///
    /// # Panics
    ///
    /// Panics, if `index` is out of index range.
    pub fn remove(&mut self, index: usize) -> bool {
        assert!(Self::is_in_range(index), "{index} is out of index range!");

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
            let existed = data_block.mask_mut().set_bit_unchecked::<false>(data_index);

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

    /// `SELF_IS_EMPTY` is true if we can GUARANTEE that self is empty.
    #[inline]
    fn unite_impl<Other, const SELF_IS_EMPTY: bool>(&mut self, other: Other)
    where
        Other: BitSetInterface<Conf=Conf>
    {
        if SELF_IS_EMPTY{ debug_assert!(self.is_empty()) }

        // 1. For TRUSTED_HIERARCHY we can upfront Insert lvl1 blocks that `self` does not have.
        //    In one go.
        if Other::TRUSTED_HIERARCHY {
            let new_lvl0_mask = self.level0_mask() | other.level0_mask();
            let mask_diff = self.level0_mask() ^ new_lvl0_mask;
            self.level1.reserve_for(new_lvl0_mask.count_ones());
            mask_diff.for_each_bit(|idx| {
                let block_index = if SELF_IS_EMPTY {
                    self.level1.push_block(Default::default())
                } else {
                    self.level1.insert_empty_block()
                };
                let item = Primitive::from_usize(block_index);
                unsafe{ self.level0.insert_unchecked_no_mask(idx, item); }
            });
            unsafe{
                *self.level0.mask_mut() = new_lvl0_mask;
            }
        }

        // Reserve data blocks.
        {
            let size_hint = crate::ops::Or::data_blocks_size_hint::<Conf>(
                self.data_blocks_size_hint(),
                other.data_blocks_size_hint()
            );
            self.data.reserve_for(size_hint.0);
        }

        let mut other_iter_state = other.make_iter_state();

        // Traverse Lvl0
        other.level0_mask().for_each_bit(|lvl0_idx|{
            let mut other_level1_block_data = MaybeUninit::uninit();
            let (other_lvl1_mask, _) = unsafe{
                other.init_level1_block_data(
                    &mut other_iter_state,
                    &mut other_level1_block_data,
                    lvl0_idx
                )
            };
            let mut other_level1_block_data = unsafe{
                other_level1_block_data.assume_init()
            };

            let this_lvl1_block_index = unsafe {
                if Other::TRUSTED_HIERARCHY {
                    // SAFETY: We just inserted all missing lvl1 blocks.
                    self.level0.get_or_zero(lvl0_idx)
                } else {
                    self.level0.get_or_insert(lvl0_idx, ||{
                        let block_index = self.level1.insert_empty_block();
                        Primitive::from_usize(block_index)
                    })
                }.as_usize()
            };
            let this_lvl1_block = unsafe {
                self.level1.blocks_mut().get_unchecked_mut(this_lvl1_block_index)
            };


            // Traverse Lvl1
            {
                let this_lvl1_mask = *this_lvl1_block.mask();
                let new_lvl1_mask = other_lvl1_mask | this_lvl1_mask;

                // I. Insert data blocks that `self` does not have as direct copy from `other`.
                let mask_diff = this_lvl1_mask ^ new_lvl1_mask;
                if Other::TRUSTED_HIERARCHY{
                    self.data.reserve_for(new_lvl1_mask.count_ones());
                }
                mask_diff.for_each_bit(|lvl1_idx|{
                    let other_data = unsafe{
                        Other::data_mask_from_block_data(
                            &mut other_level1_block_data,
                            lvl1_idx
                        )
                    };

                    if Other::TRUSTED_HIERARCHY // Always non-zero in TRUSTED_HIERARCHY
                    || !other_data.is_zero() {
                        let block_index = {
                            let block = unsafe{
                                Block::from_parts(other_data, Default::default())
                            };
                            if SELF_IS_EMPTY{
                                self.data.push_block(block)
                            } else {
                                self.data.insert_block(block)
                            }
                        };
                        let item = Primitive::from_usize(block_index);
                        unsafe{
                            if Other::TRUSTED_HIERARCHY {
                                // We'll update mask in one go in the end.
                                this_lvl1_block.insert_unchecked_no_mask(lvl1_idx, item);
                            }else {
                                this_lvl1_block.insert_unchecked(lvl1_idx, item);
                            }
                        }
                    }
                });
                if Other::TRUSTED_HIERARCHY {
                    unsafe{
                        *this_lvl1_block.mask_mut() = new_lvl1_mask;
                    }
                }

                // II. Insert intersecting blocks.
                if !SELF_IS_EMPTY{  // Can't happened if self is empty
                    let mask_intersect = this_lvl1_mask & other_lvl1_mask;
                    mask_intersect.for_each_bit(|lvl1_idx|{
                        let other_data = unsafe{
                            Other::data_mask_from_block_data(
                                &mut other_level1_block_data,
                                lvl1_idx
                            )
                        };
                        let this_data = unsafe {
                            let index = this_lvl1_block.get_or_zero(lvl1_idx);
                            self.data.blocks_mut().get_unchecked_mut(index.as_usize())
                        };
                        unsafe {
                            *this_data.mask_mut() |= other_data
                        };
                    });
                }

                // III. Remove if needed
                if !Other::TRUSTED_HIERARCHY    // Can't happened with TRUSTED_HIERARCHY
                && this_lvl1_block.mask().is_zero() {
                    // It is faster to directly write to allocated block in lvl1,
                    // and return it to pool if empty,
                    // then to use tmp block and then copy it if non-empty.
                    unsafe{
                        self.level1.remove_empty_block_unchecked(this_lvl1_block_index);
                        self.level0.remove_unchecked(lvl0_idx);
                    }
                }
            }
        });

        unsafe{ other.drop_iter_state(&mut ManuallyDrop::new(other_iter_state)); }
    }

    /// In-place union with any [BitSetInterface].
    pub fn unite<Other>(&mut self, other: Other)
    where
        Other: BitSetInterface<Conf=Conf>
    {
        self.unite_impl::<Other, false>(other)
    }

    /// Union smaller `BitSet` into bigger.
    ///
    /// Basically same as [`unite`] but auto select union direction to reduce
    /// amount of inserted data blocks, and can work with `BitSet` only.
    ///
    /// [`unite`]: Self::unite
    pub fn into_union(self, other: Self) -> Self{
        let mut left : Self;
        let right: &Self;
        // Unite into bigger bitset.
        if self.data.len() > other.data.len() {
            left  = self;
            right = &other;
        } else {
            left  = other;
            right = &self;
        }
        left.unite(right);
        left
    }

    /// In-place intersection with any [BitSetInterface].
    ///
    /// This is `O(N+M)` operation, where:
    /// * `N` is amount of blocks to be removed from `self`.
    /// * `M` is amount of blocks to be modified.
    ///
    /// So [`intersection()`] + [`materialization`] can be faster, then `intersect()`.
    /// Since `M` is equal in both cases, but with [`intersection()`] + [`materialization`]
    /// `N` is always zero (but `M` more costly, since it needs to allocate blocks).
    ///
    /// [`intersection()`]: BitSetInterface::intersection
    /// [`materialization`]: crate#laziness-and-materialization
    pub fn intersect<Other>(&mut self, other: Other)
    where
        Other: BitSetInterface<Conf=Conf>
    {
        let clear_lvl1_block = |this: &mut BitSet<Conf>, lvl1_block_idx: usize| unsafe{
            let lvl1_block = this.level1.blocks_mut().get_unchecked_mut(lvl1_block_idx);
            let lvl1_mask = *lvl1_block.mask();
            lvl1_mask.for_each_bit(|lvl1_idx| {
                let data_block_idx = lvl1_block.get_or_zero(lvl1_idx).as_usize();
                // We don't clear block, since that will clear only it's mask.
                // Mask will be cleared any way on pop_empty_block()
                //this.data.blocks_mut().get_unchecked_mut(data_block_idx).clear();
                this.data.remove_empty_block_unchecked(data_block_idx);
                lvl1_block.remove_unchecked_no_mask(lvl1_idx);
            });
            *lvl1_block.mask_mut() = BitBlock::zero();
        };

        // 1. Roughly cut by lvl0 mask
        let other_lvl0_mask = other.level0_mask();
        let new_lvl0_mask = *self.level0.mask() & other_lvl0_mask;
        {
            let mask_diff = *self.level0.mask() ^ new_lvl0_mask;
            mask_diff.for_each_bit(|lvl0_idx| unsafe{
                let lvl1_block_idx = self.level0.get_or_zero(lvl0_idx).as_usize();
                clear_lvl1_block(self, lvl1_block_idx);
                self.level1.remove_empty_block_unchecked(lvl1_block_idx);
                self.level0.remove_unchecked_no_mask(lvl0_idx);
            });
            unsafe{ *self.level0.mask_mut() = new_lvl0_mask }
        }

        let mut other_iter_state = other.make_iter_state();

        // Traverse Lvl0 intersection
        new_lvl0_mask.for_each_bit(|lvl0_idx|{
            let mut other_level1_block_data = MaybeUninit::uninit();
            let (other_lvl1_mask, _) = unsafe{
                other.init_level1_block_data(
                    &mut other_iter_state,
                    &mut other_level1_block_data,
                    lvl0_idx
                )
            };
            let mut other_level1_block_data = unsafe{
                other_level1_block_data.assume_init()
            };

            let this_lvl1_block_index = unsafe {
                // SAFETY: We know we have it - it's intersection
                self.level0.get_or_zero(lvl0_idx).as_usize()
            };

            let this_lvl1_block = unsafe {
                self.level1.blocks_mut().get_unchecked_mut(this_lvl1_block_index)
            };

            let this_data = &mut self.data;

            // Traverse Lvl1
            {
                let this_lvl1_mask = *this_lvl1_block.mask();
                let new_lvl1_mask = other_lvl1_mask & this_lvl1_mask;

                // I. Remove data blocks that `self` should not have.
                let mask_diff = this_lvl1_mask ^ new_lvl1_mask;
                mask_diff.for_each_bit(|lvl1_idx| unsafe{
                    let this_data_idx = this_lvl1_block.get_or_zero(lvl1_idx).as_usize();
                    // We don't clear block, since that will clear only it's mask.
                    // Mask will be cleared any way on pop_empty_block()
                    /*let this_data_block = this_data.blocks_mut().get_unchecked_mut(this_data_idx);
                    this_data_block.clear(); */
                    this_data.remove_empty_block_unchecked(this_data_idx);
                    this_lvl1_block.remove_unchecked_no_mask(lvl1_idx);
                });
                unsafe{
                    *this_lvl1_block.mask_mut() ^= mask_diff;
                }

                // II. Do actual data intersection
                new_lvl1_mask.for_each_bit(|lvl1_idx| unsafe{
                    let other_data =
                        Other::data_mask_from_block_data(
                            &mut other_level1_block_data,
                            lvl1_idx
                        );
                    let this_data_idx = this_lvl1_block.get_or_zero(lvl1_idx).as_usize();
                    let this_data_block = this_data.blocks_mut().get_unchecked_mut(this_data_idx);

                    *this_data_block.mask_mut() &= other_data;

                    if this_data_block.mask().is_zero(){
                        this_data.remove_empty_block_unchecked(this_data_idx);
                        this_lvl1_block.remove_unchecked(lvl1_idx);
                    }
                });

                // III. Remove if needed
                if this_lvl1_block.mask().is_zero() {
                    unsafe{
                        self.level1.remove_empty_block_unchecked(this_lvl1_block_index);
                        self.level0.remove_unchecked(lvl0_idx);
                    }
                }
            }
        });

        unsafe{ other.drop_iter_state(&mut ManuallyDrop::new(other_iter_state)); }
    }

    /// Intersect bigger `BitSet` into smaller.
    ///
    /// Basically same as [`intersect`] but auto select intersection direction
    /// to reduce amount of removed data blocks, and can work with `BitSet` only.
    ///
    /// [`intersect`]: Self::intersect
    pub fn into_intersection(self, other: Self) -> Self {
        let mut left : Self;
        let right: &Self;
        // Intersect into smaller bitset.
        if self.data.len() < other.data.len() {
            left  = self;
            right = &other;
        } else {
            left  = other;
            right = &self;
        }
        left.intersect(right);
        left
    }
}

impl<Conf: Config> BitSetBase for BitSet<Conf> {
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = true;
}

impl<Conf: Config> LevelMasks for BitSet<Conf> {
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

    #[inline]
    fn data_blocks_size_hint(&self) -> crate::ops::SizeHint {
        // One empty block always reserved.
        let len = self.data.len() - 1;
        (len, len)
    }
}

impl<Conf: Config> LevelMasksIterExt for BitSet<Conf> {
    /// Points to elements in heap. Guaranteed to be stable.
    /// This is just plain pointers with null in default:
    /// `(*const LevelDataBlock<Conf>, *const Level1Block<Conf>)`
    type Level1BlockData = (
        Option<NonNull<LevelDataBlock<Conf>>>,  /* data array pointer */
        Option<NonNull<Level1Block<Conf>>>      /* block pointer */
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

impl_bitset!(impl<Conf> for ref BitSet<Conf> where Conf: Config);

impl<Conf, Rhs> BitOrAssign<Rhs> for BitSet<Conf>
where
    Conf: Config,
    Rhs: BitSetInterface<Conf=Conf>
{
    /// See [Self::unite].
    #[inline]
    fn bitor_assign(&mut self, rhs: Rhs) {
        self.unite(rhs);
    }
}

impl<Conf: Config> BitOr for BitSet<Conf> {
    type Output = Self;

    /// See [Self::into_union].
    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        self.into_union(rhs)
    }
}

impl<Conf, Rhs> BitAndAssign<Rhs> for BitSet<Conf>
where
    Conf: Config,
    Rhs: BitSetInterface<Conf=Conf>
{
    /// See [Self::intersect].
    #[inline]
    fn bitand_assign(&mut self, rhs: Rhs) {
        self.intersect(rhs);
    }
}

impl<Conf: Config> BitAnd for BitSet<Conf> {
    type Output = Self;

    /// See [Self::into_intersection].
    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        self.into_intersection(rhs)
    }
}