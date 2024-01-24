use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::ControlFlow;
use crate::{assume, level_indices};
use crate::bit_block::BitBlock;
use crate::config::{DefaultBlockIterator, Config, DefaultIndexIterator};

// We have this separate trait with Config, to avoid making LevelMasks public.
pub trait BitSetBase {
    type Conf: Config;
    
    /// Does this bitset have `TrustedHierarchy`?
    const TRUSTED_HIERARCHY: bool;
}

/// Basic interface for accessing block masks. Can work with [SimpleIter].
/// 
/// [SimpleIter]: crate::iter::SimpleBlockIter
pub trait LevelMasks: BitSetBase{
    fn level0_mask(&self) -> <Self::Conf as Config>::Level0BitBlock;

    /// # Safety
    ///
    /// index is not checked
    unsafe fn level1_mask(&self, level0_index: usize)
        -> <Self::Conf as Config>::Level1BitBlock;

    /// # Safety
    ///
    /// indices are not checked
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize)
        -> <Self::Conf as Config>::DataBitBlock;
}

/// More sophisticated masks interface, optimized for iteration speed of 
/// generative/lazy bitset.
/// 
/// For example, in [Reduce] this achieved through
/// caching level1(pre-data level) block pointers of all sets. Which also allows to discard
/// bitsets with empty level1 blocks in final stage of getting data blocks.
/// Properly implementing this gave [Reduce] and [Apply] 25-100% performance boost.  
///
/// NOTE: This interface is somewhat icky and initially was intended for internal use.
/// I don't know if it will be actually used, so no work is done on top of that.
/// If you do use it, and want it better - open an issue.
/// 
/// # How it is used
/// 
/// See [CachingBlockIter::next()] code to see how it used.   
/// 
/// ```[ignore]
/// let mut state = bitset.make_iter_state();
/// let mut level1_block_data = MaybeUninit::new(Default::default());
/// 
/// fn next() {
///     ...
///     level1_block_data.assume_init_drop();
///     let (level1_mask, is_not_empty) = bitset.update_level1_block_data(state, level1_block_data, level0_index);
///     ...
///     let bitblock = data_mask_from_block_data(level1_block_data, level1_index);
///     
///     return bitblock;
/// }
/// 
/// level1_block_data.assume_init_drop();
/// bitset.drop_iter_state(state);
/// ```
/// 
/// [Reduce]: crate::Reduce
/// [Apply]: crate::Apply
/// [CachingBlockIter::next()]: crate::iter::CachingBlockIter::next()
pub trait LevelMasksIterExt: LevelMasks{
    /// Consists from child states (if any) + Self state.
    /// 
    /// You may need this, since [Level1BlockData] must be POD.
    /// Use `()` for stateless.
    /// 
    /// [Level1BlockData]: Self::Level1BlockData
    type IterState;

    /// Level1 block related data, used to speed up data_mask access.
    ///
    /// Prefer POD, or any kind of drop-less. 
    /// 
    /// In library, used to cache Level1Block(s) for faster DataBlock access,
    /// without traversing whole hierarchy for getting each block during iteration.
    type Level1BlockData: Default;

    fn make_iter_state(&self) -> Self::IterState;
    
    /// Having separate function for drop not strictly necessary, since
    /// IterState can actually drop itself. But! This allows not to store cache
    /// size within IterState. Which makes FixedCache CacheData ZST, if its childs
    /// are ZSTs, and which makes cache construction and destruction noop. Which is
    /// important for short iteration sessions.
    /// 
    /// P.S. This can be done at compile-time by opting out "len" counter,
    /// but stable Rust does not allow to do that yet.
    /// 
    /// # Safety
    /// 
    /// - `state` must not be used after this.
    /// - Must be called exactly once for each `state`.
    unsafe fn drop_iter_state(&self, state: &mut ManuallyDrop<Self::IterState>);

    /// Init `level1_block_data` and return (Level1Mask, is_not_empty).
    /// 
    /// `level1_block_data` will come in undefined state - rewrite it completely.
    ///
    /// `is_not_empty` is not used by iterator itself, but can be used by other 
    /// generative bitsets (namely [Reduce]) - we expect compiler to optimize away non-used code.
    /// It exists - because sometimes you may have faster ways of checking emptiness,
    /// then checking simd register (bitblock) for zero in general case.
    /// For example, in BitSet - it is done by checking of block indirection index for zero.
    /// 
    /// # Safety
    ///
    /// indices are not checked.
    /// 
    /// [Reduce]: crate::Reduce
    // Performance-wise it is important to use this in-place construct style, 
    // instead of just returning Level1BlockData. Even if we return Level1BlockData,
    // and then immoderately write it to MaybeUninit - compiler somehow still can't
    // optimize it as direct memory write without intermediate bitwise copy.
    unsafe fn init_level1_block_data(
        &self,
        state: &mut Self::IterState,
        level1_block_data: &mut MaybeUninit<Self::Level1BlockData>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool);

    /// # Safety
    ///
    /// indices are not checked.
    unsafe fn data_mask_from_block_data(
        level1_block_data: &Self::Level1BlockData, level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock;
}

impl<'a, T: LevelMasks> BitSetBase for &'a T {
    type Conf = T::Conf;
    const TRUSTED_HIERARCHY: bool = T::TRUSTED_HIERARCHY;
}
impl<'a, T: LevelMasks> LevelMasks for &'a T {
    #[inline]
    fn level0_mask(&self) -> <Self::Conf as Config>::Level0BitBlock {
        <T as LevelMasks>::level0_mask(self)
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize)
        -> <Self::Conf as Config>::Level1BitBlock
    {
        <T as LevelMasks>::level1_mask(self, level0_index)
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize)
        -> <Self::Conf as Config>::DataBitBlock
    {
        <T as LevelMasks>::data_mask(self, level0_index, level1_index)
    }
}
impl<'a, T: LevelMasksIterExt> LevelMasksIterExt for &'a T {
    type Level1BlockData = T::Level1BlockData;

    type IterState = T::IterState;

    #[inline]
    fn make_iter_state(&self) -> Self::IterState {
        <T as LevelMasksIterExt>::make_iter_state(self)
    }

    #[inline]
    unsafe fn drop_iter_state(&self, cache: &mut ManuallyDrop<Self::IterState>) {
        <T as LevelMasksIterExt>::drop_iter_state(self, cache)
    }

    #[inline]
    unsafe fn init_level1_block_data(
        &self,
        state: &mut Self::IterState,
        level1_blocks: &mut MaybeUninit<Self::Level1BlockData>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        <T as LevelMasksIterExt>::init_level1_block_data(
            self, state, level1_blocks, level0_index
        )
    }

    #[inline]
    unsafe fn data_mask_from_block_data(
        level1_blocks: &Self::Level1BlockData, level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock {
        <T as LevelMasksIterExt>::data_mask_from_block_data(
            level1_blocks, level1_index
        )
    }
}

// User-side interface
/// Bitset interface.
/// 
/// Implemented for bitset references and optionally for values. 
/// So as argument - accept BitSetInterface by value.
/// _(Act as kinda forwarding reference in C++)_
/// 
/// # Traversing
/// 
/// [CachingBlockIter] and [CachingIndexIter] have specialized `for_each()` implementation and `traverse()`.
/// 
/// Like with most Rust iterators, traversing[^traverse_def] is somewhat faster
/// then iteration. In this particular case, it has noticeable difference in micro-benchmarks.
/// Remember, that iteration is already super-fast, and any tiny change become important at that scale.
/// Hence, this will have effect in really tight loops (like incrementing counter).
///
/// [^traverse_def]: Under "traverse" we understand function application for 
/// each element of bitset.
/// 
/// # Implementation
/// 
/// Consider using [impl_bitset!] instead of implementing it manually.
///
/// Implementing BitSetInterface for T will make it passable by value to [apply], [reduce].
/// That may be not what you want, if your type contains heavy data, or your
/// [LevelMasksIterExt] implementation depends on *Self being stable during iteration.
/// If that is the case - implement only for &T.
/// 
/// [CachingBlockIter]: crate::iter::CachingBlockIter
/// [CachingIndexIter]: crate::iter::CachingIndexIter
/// [LevelMasksIterExt]: crate::internals::LevelMasksIterExt
/// [impl_bitset!]: crate::impl_bitset!
/// [apply]: crate::apply()
/// [reduce]: crate::reduce()
pub unsafe trait BitSetInterface
    : BitSetBase 
    + LevelMasksIterExt 
    + IntoIterator<IntoIter = DefaultIndexIterator<Self>>
    + Sized
{
    #[inline]
    fn block_iter(&self) -> DefaultBlockIterator<&'_ Self> {
        DefaultBlockIterator::new(self)
    }

    #[inline]
    fn iter(&self) -> DefaultIndexIterator<&'_ Self> {
        DefaultIndexIterator::new(self)
    }
    
    #[inline]
    fn into_block_iter(self) -> DefaultBlockIterator<Self> {
        DefaultBlockIterator::new(self)
    }
    
    #[inline]
    fn contains(&self, index: usize) -> bool {
        bitset_contains(self, index)
    } 
    
    /// O(1) if [TRUSTED_HIERARCHY], O(N) otherwise.
    /// 
    /// [TRUSTED_HIERARCHY]: BitSetBase::TRUSTED_HIERARCHY
    #[inline]
    fn is_empty(&self) -> bool {
        bitset_is_empty(self)
    }
}

#[inline]
pub(crate) fn bitset_contains<S: LevelMasks>(bitset: S, index: usize) -> bool {
    let (level0_index, level1_index, data_index) = 
        level_indices::<S::Conf>(index);
    unsafe{
        let data_block = bitset.data_mask(level0_index, level1_index);
        data_block.get_bit(data_index)
    }
} 

pub(crate) fn bitset_is_empty<S: LevelMasksIterExt>(bitset: S) -> bool {
    if S::TRUSTED_HIERARCHY{
        return bitset.level0_mask().is_zero();
    }
    
    use ControlFlow::*;
    DefaultBlockIterator::new(bitset).traverse(|block|{
        if block.is_empty(){
            Break(())
        } else {
            Continue(())
        }
    }).is_break()
}

/// Optimistic depth-first check.
/// 
/// This traverse-based implementation is faster then using two iterators.
pub(crate) fn bitsets_eq<L, R>(left: L, right: R) -> bool
where
    L: LevelMasksIterExt,
    R: LevelMasksIterExt<Conf = L::Conf>,
{
    let left_level0_mask  = left.level0_mask();
    let right_level0_mask = right.level0_mask();

    // We can do early return with TrustedHierarchy. 
    /*const*/ let is_trusted_hierarchy = L::TRUSTED_HIERARCHY & R::TRUSTED_HIERARCHY;
    
    let level0_mask = 
        if is_trusted_hierarchy{
            if left_level0_mask != right_level0_mask {
                return false;
            }  
            left_level0_mask
        } else {
            // skip only 0's on both sides
            left_level0_mask | right_level0_mask
        };
    
    let mut left_cache_data  = left.make_iter_state();
    let mut right_cache_data = right.make_iter_state();
    
    let mut left_level1_blocks  = MaybeUninit::new(Default::default());
    let mut right_level1_blocks = MaybeUninit::new(Default::default());
    
    use ControlFlow::*;
    let is_eq = level0_mask.traverse_bits(|level0_index|{
        let (left_level1_mask, left_valid) = unsafe {
            left_level1_blocks.assume_init_drop();
            left.init_level1_block_data(&mut left_cache_data, &mut left_level1_blocks, level0_index)
        };
        let (right_level1_mask, right_valid) = unsafe {
            right_level1_blocks.assume_init_drop();
            right.init_level1_block_data(&mut right_cache_data, &mut right_level1_blocks, level0_index)
        };
        
        if is_trusted_hierarchy {
            unsafe{ 
                assume!(left_valid);
                assume!(right_valid);
            }
            if left_level1_mask != right_level1_mask {
                return Break(());
            }
        }
        
        if is_trusted_hierarchy || (left_valid & right_valid) {
            let level1_mask =
                if is_trusted_hierarchy {
                    left_level1_mask
                } else{
                    left_level1_mask | right_level1_mask
                };
            
            level1_mask.traverse_bits(|level1_index|{
                let left_data = unsafe {
                    L::data_mask_from_block_data(left_level1_blocks.assume_init_ref(), level1_index)
                };
                let right_data = unsafe {
                    R::data_mask_from_block_data(right_level1_blocks.assume_init_ref(), level1_index)
                };
                
                if left_data == right_data{
                    Continue(())
                }  else {
                    Break(())                 
                }
            })            
        } else if left_valid /*right is zero*/ {
            if L::TRUSTED_HIERARCHY{
                return if left_level1_mask.is_zero() {
                    Continue(())
                } else {
                    Break(())
                }
            }
            
            left_level1_mask.traverse_bits(|level1_index|{
                let left_data = unsafe{
                    L::data_mask_from_block_data(left_level1_blocks.assume_init_ref(), level1_index)
                };
                if left_data.is_zero() {
                    Continue(())
                }  else {
                    Break(())                 
                }                
            })
        } else if right_valid /*left is zero*/ {
            if R::TRUSTED_HIERARCHY{
                return if right_level1_mask.is_zero() {
                    Continue(())
                } else {
                    Break(())
                }
            }
            
            right_level1_mask.traverse_bits(|level1_index|{
                let right_data = unsafe{
                    R::data_mask_from_block_data(right_level1_blocks.assume_init_ref(), level1_index)
                };
                if right_data.is_zero() {
                    Continue(())
                }  else {
                    Break(())                 
                }                
            })            
        } else {
            // both are empty - its ok - just move on.
            Continue(())
        }
    }).is_continue();
    
    unsafe {
        left_level1_blocks.assume_init_drop();
        right_level1_blocks.assume_init_drop();
    }
    
    is_eq
}