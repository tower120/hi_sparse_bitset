use std::mem::{ManuallyDrop, MaybeUninit};
use crate::block::Block;
use crate::compact_block::CompactBlock;
use crate::config::{Config, SmallConfig};
use crate::{BitSetBase, internals};
use crate::bitset_interface::{LevelMasks, LevelMasksIterExt};
use crate::raw::RawBitSet;

type Level0Block<Conf> = Block<
    <Conf as Config>::Level0BitBlock, 
    <Conf as Config>::Level0BlockIndices
>;
type Level1Block<Conf> = CompactBlock<
    <Conf as Config>::Level1BitBlock,
    <Conf as SmallConfig>::Level1MaskU64Populations,
    <Conf as Config>::Level1BlockIndices,
    <Conf as SmallConfig>::Level1SmallBlockIndices,
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

/// Same as [BitSet], but sparsely populated hierarchy blocks 9 times smaller.
/// 
/// Which means that sparse sets virtually do not have indirection-related memory overhead!
/// 
/// # Memory
/// 
/// For [_128bit] each Level1 block consumes just 32 bytes, if less
/// than 8 data blocks pointed from. 256+32 bytes otherwise.   
/// 
/// # Performance
/// 
/// All operations still have O(1) complexity. But in terms of raw performance,
/// (due to additional layer of indirection)
/// this is x1.5 - x2 slower than `BitSet`. Which is still very fast.
/// 
/// # Implementation details
/// 
/// ```text
/// Level0          128bit SIMD                                        
///                  [u8;128]                                          
///
///             ┌   128bit SIMD    ┐    ╭─ ── ── ── ── ── ── ── ── ── ╮
///             │ ╭──────────────╮ │      Act as SBO:                 │
/// Level1   Vec│ │   [u16;7]    │ │    │- Inline SparseBitMap, for    
///             │ │ ──────────── │◁┼────┤small size.                  │
///             │ │Box<[u16;128]>│ │     - Boxed full-size array with │
///             └ ╰──────────────╯ ┘    │direct access for a big one.    
///             ┌                  ┐    ╰ ── ── ── ── ── ── ── ── ── ─╯
/// Data     Vec│   128bit SIMD    │                                   
///             └                  ┘                                   
/// ```
/// SparseBitMap - `bit_block` acts as a sparse array:
/// ```text
///                   0 1       2 3          ◁═ popcnt before element
///                                                                  
///  bit_block      0 1 1 0 0 0 1 1 0 0 ...                          
///               └───┬─┬───────┬─┬─────────┘                        
///                ┌──┘┌┘ ┌─────┘ │                                  
///                │   │  │  ┌────┘                                  
///                ▼   ▼  ▼  ▼                                       
/// dense_array    1, 32, 4, 5               len = bit_block popcnt  
/// ```
/// As you can see, SparseBitMap has fast O(1) per-index access.
/// Insert and remove - are O(N) operations, because
/// `dense_array` must keep its element order.
/// 
/// Why not just use SparseBitMap for everything?
/// Big-sized `dense_array` should be placed somewhere outside of LevelBlock struct 
/// to be memory efficient (e.g., in dynamic-sized heap). Hence - it 
/// will be accessed through the pointer (which is yet another layer of indirection).
/// Due to this, and the O(N) insert/remove - SparseBitMap is used only for small-sized arrays.
/// We aim for a 16-element array - as with this size, data blocks pointed 
/// from level1 block will have the same size in total, as a full-sized level1 block 
/// indirection array.  
/// 
pub struct SmallBitSet<Conf: SmallConfig>(
    RawSmallBitSet<Conf>
);

impl<Conf: SmallConfig> Clone for SmallBitSet<Conf> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Conf: SmallConfig> Default for SmallBitSet<Conf> {
    #[inline]
    fn default() -> Self{
        Self(Default::default())
    }
}

impl<Conf: SmallConfig> FromIterator<usize> for SmallBitSet<Conf> {
    #[inline]
    fn from_iter<T: IntoIterator<Item=usize>>(iter: T) -> Self {
        Self(RawSmallBitSet::<Conf>::from_iter(iter))
    }
}

impl<Conf: SmallConfig, const N: usize> From<[usize; N]> for SmallBitSet<Conf> {
    #[inline]
    fn from(value: [usize; N]) -> Self {
        Self(RawSmallBitSet::<Conf>::from(value))
    }
}

impl<Conf: SmallConfig> SmallBitSet<Conf> {
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

impl<Conf: SmallConfig> BitSetBase for SmallBitSet<Conf>{
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = RawSmallBitSet::<Conf>::TRUSTED_HIERARCHY;
}

impl<Conf: SmallConfig> LevelMasks for SmallBitSet<Conf>{
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

impl<Conf: SmallConfig> LevelMasksIterExt for SmallBitSet<Conf>{
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

internals::impl_bitset!(impl<Conf> for ref SmallBitSet<Conf> where Conf: SmallConfig);