use core::slice;
use std::{
    mem::MaybeUninit, ops::Deref, ptr::{NonNull, copy_nonoverlapping}, rc::Rc, sync::Arc
};
use crate::{
    BitBlock, BitSetBase,
    bit_utils::{get_bit_unchecked, zero_high_bits_unchecked},
    config::Config,
    internals::{LevelMasks, LevelMasksIterExt, impl_bitset},
    primitive::Primitive,
    primitive_array::PrimitiveArray,
    serialization::{lvl0_padding, lvl1_padding}
};

type Lvl0Mask<Conf> = <Conf as Config>::Level0BitBlock;
type Lvl1Mask<Conf> = <Conf as Config>::Level1BitBlock;
type DataMask<Conf> = <Conf as Config>::DataBitBlock;

type Lvl0Index<Conf> = <<Conf as Config>::Level0BlockIndices as PrimitiveArray>::Item;
type Lvl1Index<Conf> = <<Conf as Config>::Level1BlockIndices as PrimitiveArray>::Item;

/// In bytes.
const ROOT_MASK_MAX_SIZE: usize = 32;

/// Data source for [DirectBitset].
pub trait DirectDataSource{
    /// This must be no-op or VERY cheap operation.
    fn data_src(&self) -> &[u8];
}

impl<T: AsRef<[u8]>> DirectDataSource for Arc<T>{
    #[inline]
    fn data_src(&self) -> &[u8] {
        self.deref().as_ref()
    }
}

impl<T: AsRef<[u8]>> DirectDataSource for Rc<T>{
    #[inline]
    fn data_src(&self) -> &[u8] {
        self.deref().as_ref()
    }
}

impl DirectDataSource for &[u8]{
    #[inline]
    fn data_src(&self) -> &[u8] {
        self
    }
}

impl DirectDataSource for Vec<u8>{
    #[inline]
    fn data_src(&self) -> &[u8] {
        self
    }
}

/// Bitset that work directly with any source of [`serialized data`].
///
/// Have very small additional memory overhead, consisting from lvl0 and lvl1 masks.
/// Constructing `DirectBitset` is MUCH faster then constructing [`BitSet`].
///
/// [`serialized data`]: crate#serialization
/// [`BitSet`]: crate::BitSet
///
/// # Aligning
///
/// `DirectBitset` can benefit from aligned data performance-wise.
/// Serialized data already perfectly aligned. You need only to provide
/// correct "base" and set generic argument `ALIGNED` to true.
///
/// Base address must be aligned to [`Conf::MAX_MASK_ALIGN`]. You can achieve this
/// by using something like `aligned_vec` crate. Memory-mapped file almost for
/// sure will have greater base align - so it should work as-is.
///
/// N.B. On most desktop platforms unaligned reads have negligible
/// performance overhead.
///
/// [`Conf::MAX_MASK_ALIGN`]: crate::config::Config::MAX_MASK_ALIGN
///
/// # Example
///
/// With memory-mapped file:
/// ```
/// # use std::sync::Arc;
/// # use hi_sparse_bitset::{config, BitSet, DirectBitset};
/// use memmap2::Mmap;
///
/// // We can use `ALIGN=true` here since we know that Mmap already aligned.
/// type MmapBitset<Conf> = DirectBitset<Conf, Arc<Mmap>, true>;
/// type Conf = config::_64bit;
///
/// // Make serialized data in tempfile.
/// let mut file = tempfile::tempfile().unwrap();
/// BitSet::<Conf>::from(
///     [1,2,3,4,66,100,16089]
/// ).serialize(&mut file).unwrap();
///
/// // Mmap file.
/// let mmap = unsafe { Mmap::map(&file).unwrap()  };
///
/// // Feed mmaped file to DirectBitset.
/// let bitset: MmapBitset<Conf> = MmapBitset::new(Arc::new(mmap), 0).unwrap();
/// ```
///
/// With aligning:
///
///```
/// # use hi_sparse_bitset::{config, config::Config, BitSet, DirectBitset};
/// use aligned_vec::{AVec, ConstAlign};
///
/// type Conf = config::_64bit;
/// const ALIGN: usize = <Conf as Config>::MAX_MASK_ALIGN;
/// type AlignedVec = AVec<u8, ConstAlign<ALIGN>>;
///
/// // Serialize to Vec.
/// let mut vec = Vec::new();
/// BitSet::<Conf>::from(
///     [1,2,3,4,66,100,16089]
/// ).serialize(&mut vec).unwrap();
///
/// // We need to make sure, that byte array have aligned base.
/// // We use AVec for this. Since AVec doesn't implement Write yet,
/// // we just copy byte array in it.
/// let avec = AlignedVec::from_slice(ALIGN, &vec);
///
/// let im = DirectBitset::<Conf, &[u8], true>::new(&avec, 0).unwrap();
/// ```
#[derive(Clone)]
pub struct DirectBitset<Conf: Config, Data, const ALIGNED: bool = false>{
    lvl0_mask: Lvl0Mask<Conf>,
    lvl0_u64_index_starts: [Lvl0Index<Conf>; ROOT_MASK_MAX_SIZE/8],
    // We can't read this directly from data, since we need correct endianess,
    // because we work with u64 sub-masks.
    lvl1_masks: Vec<Lvl1Mask<Conf>>,
    lvl1_u64_index_starts: Vec<Lvl1Index<Conf>>,
    data: Data,
    data_offset: usize,
    data_blocks_len: usize,
}

#[inline]
unsafe fn read_mask<Mask: BitBlock, const ALIGNED: bool>(ptr: *const u8) -> Mask {
    #[cfg(target_endian = "little")]
    if ALIGNED{
        return ptr.cast::<Mask>().read();
    }

    let mut bytes: MaybeUninit<Mask::BytesArray> = MaybeUninit::uninit();
    if ALIGNED{
        // cast to mask
        copy_nonoverlapping(
            ptr.cast(),
            bytes.as_mut_ptr(),
            size_of::<Mask>()
        );
    } else {
        // cast to bytes
        copy_nonoverlapping(
            ptr,
            bytes.as_mut_ptr().cast::<u8>(),
            size_of::<Mask>()
        );
    }
    Mask::from_le_bytes(bytes.assume_init())
}

#[inline]
fn ptr_is_aligned_to<T>(ptr: *const T, align: usize) -> bool {
    if !align.is_power_of_two() {
        panic!("is_aligned_to: align is not a power-of-two");
    }

    ptr.addr() & (align - 1) == 0
}

impl<Conf: Config, Data: DirectDataSource, const ALIGNED: bool> DirectBitset<Conf, Data, ALIGNED> {
    // TODO: use from immutable_bitset
    #[inline]
    fn lvl1_as_u64(slice: &[Lvl1Mask<Conf>]) -> &[u64]{
        unsafe {
            slice::from_raw_parts(
                slice.as_ptr().cast(),
                slice.len() * (size_of::<Lvl1Mask<Conf>>() / 8)
            )
        }
    }

    /// * `data` - data source that points to byte data.
    /// * `offset` - `data` offset in bytes, where serialized data begins.
    ///
    /// For `ALIGNED`, DirectBitset `data` + `offset` must be aligned to MAX_MASK_ALIGN,
    /// otherwise error will be returned.
    pub fn new(data: Data, offset: usize) -> std::io::Result<Self> {
        const{ assert!(size_of::<Lvl0Mask<Conf>>() <= ROOT_MASK_MAX_SIZE) }

        let slice = &data.data_src()[offset..];
        let mut ptr = slice.as_ptr();
        let mut len = slice.len();

        if ALIGNED {
            let aligned = ptr_is_aligned_to(ptr, Conf::MAX_MASK_ALIGN);
            if !aligned{
                use std::io::*;
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "data start base must be aligned with Conf::MAX_MASK_ALIGN",
                ));
            }
        }

        // I. Lvl0
        let lvl0_mask_size = size_of::<Lvl0Mask<Conf>>();
        if len < lvl0_mask_size {
            use std::io::*;
            return Err(Error::from(ErrorKind::InvalidData));
        }
        let lvl0_mask: Lvl0Mask<Conf> = unsafe {
            read_mask::<_, ALIGNED>(ptr)
        };
        unsafe{
            ptr = ptr.add(lvl0_mask_size);
            len -= lvl0_mask_size;
        }
        let mut lvl1_blocks_len = 0;
        let mut lvl0_u64_index_starts = [Primitive::ZERO; ROOT_MASK_MAX_SIZE/8];
        for (idx, sub_mask) in lvl0_mask.as_array().iter().enumerate(){
            unsafe{
                *lvl0_u64_index_starts.get_unchecked_mut(idx) =
                    Primitive::from_usize(lvl1_blocks_len);
            };
            lvl1_blocks_len += sub_mask.count_ones();
        }

        // lvl0padding
        unsafe{
            let pos = ptr.offset_from_unsigned(slice.as_ptr());
            let padding = lvl0_padding::<Conf>(pos);
            ptr = ptr.add(padding);
            len -= padding;
        }

        // II. Lvl1
        let lvl1_mask_size = size_of::<Lvl1Mask<Conf>>();
        let lvl1_bytes_len = lvl1_mask_size*lvl1_blocks_len;
        if len < lvl1_bytes_len {
            use std::io::*;
            return Err(Error::from(ErrorKind::InvalidData));
        }

        // Bulk copy all lvl1 masks
        let mut lvl1_masks: Vec<Lvl1Mask<Conf>> = Vec::with_capacity(lvl1_blocks_len);
        unsafe{
            #[cfg(target_endian = "little")]
            // Unaligned read.
            copy_nonoverlapping(
                ptr,
                lvl1_masks.spare_capacity_mut().as_mut_ptr().cast::<u8>(),
                lvl1_blocks_len * lvl1_mask_size
            );

            #[cfg(target_endian = "big")]
            const{ unimplemented!() }

            lvl1_masks.set_len(lvl1_blocks_len);
            ptr = ptr.add(lvl1_bytes_len);
            len -= lvl1_bytes_len;
        }

        // TODO: use from immutable_bitset
        // Calculate lvl1 index starts
        let mut lvl1_u64_index_starts = Vec::with_capacity(lvl1_blocks_len * (lvl1_mask_size/8));
        let mut data_blocks_len = 0;
        for lvl1_mask_u64 in Self::lvl1_as_u64(&lvl1_masks){
            // TODO: more efficient push
            lvl1_u64_index_starts.push(Primitive::from_usize(data_blocks_len));
            data_blocks_len += lvl1_mask_u64.count_ones();
        }

        // lvl1padding
        unsafe{
            let pos = ptr.offset_from_unsigned(slice.as_ptr());
            let padding = lvl1_padding::<Conf>(pos);
            ptr = ptr.add(padding);
            len -= padding;
        }

        // III. Data level checks
        if len < size_of::<DataMask<Conf>>() * data_blocks_len {
            use std::io::*;
            return Err(Error::from(ErrorKind::InvalidData));
        }

        let data_offset = offset + unsafe{
            ptr.offset_from_unsigned(slice.as_ptr())
        };

        Ok(Self{
            lvl0_mask,
            lvl0_u64_index_starts,
            lvl1_masks,
            lvl1_u64_index_starts,
            data,
            data_offset,
            data_blocks_len
        })
    }

    #[inline(always)]
    fn lvl_get_item<LvlMask:BitBlock>(
        offsets: &[impl Primitive],
        sub_masks: &[u64],
        sub_mask_index_offset: usize,
        index: usize
    ) -> Option<usize> {
        let u64_index;
        let bit_index;
        if LvlMask::SIZE_POT_EXPONENT > 6{
            u64_index = index / 64;
            bit_index = index % 64;
        } else {
            u64_index = 0;
            bit_index = index;
        }

        let u64_index = u64_index + sub_mask_index_offset;

        let offset = unsafe{ offsets.get_unchecked(u64_index).as_usize() };
        let sub_mask = unsafe{ *sub_masks.get_unchecked(u64_index) };
        if unsafe{ !get_bit_unchecked(sub_mask, bit_index) }{
            return None;
        }
        Some(offset + unsafe{
            zero_high_bits_unchecked(sub_mask, bit_index).count_ones() as usize
        })
    }

    #[inline]
    fn lvl0_get_item(&self, index: usize) -> Option<usize> {
        Self::lvl_get_item::<Lvl0Mask<Conf>>(
            &self.lvl0_u64_index_starts,
            self.lvl0_mask.as_array(),
            0,
            index
        )
    }

    #[inline]
    fn lvl1_get_item(&self, lvl1_block_index: usize, level1_index: usize) -> Option<usize> {
        Self::lvl_get_item::<Lvl1Mask<Conf>>(
            &self.lvl1_u64_index_starts,
            Self::lvl1_as_u64(&self.lvl1_masks),
            lvl1_block_index * (size_of::<Lvl1Mask<Conf>>() / 8),
            level1_index
        )
    }

    #[inline]
    unsafe fn data_mask(&self, data_index: usize) -> DataMask<Conf> {
        let offset_bytes = self.data_offset + data_index * size_of::<DataMask<Conf>>();
        let ptr = self.data.data_src().as_ptr().add(offset_bytes);
        read_mask::<_, ALIGNED>(ptr)
    }
}

impl<Conf: Config, Data: DirectDataSource, const ALIGNED: bool> BitSetBase for DirectBitset<Conf, Data, ALIGNED>{
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = true;
}

impl<Conf: Config, Data: DirectDataSource, const ALIGNED: bool> LevelMasks for DirectBitset<Conf, Data, ALIGNED>{
    #[inline]
    fn level0_mask(&self) -> Lvl0Mask<Conf> {
        self.lvl0_mask
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize) -> Lvl1Mask<Conf> {
        if let Some(block_index) = self.lvl0_get_item(level0_index){
            *self.lvl1_masks.get_unchecked(block_index)
        } else {
            BitBlock::zero()
        }
    }

    #[inline]
    unsafe fn data_mask(&self, level0_index: usize, level1_index: usize)
        -> <Self::Conf as Config>::DataBitBlock {
        let lvl1_block_index = match self.lvl0_get_item(level0_index){
            Some(idx) => idx,
            None => return BitBlock::zero(),
        };

        let data_index = self.lvl1_get_item(lvl1_block_index, level1_index);
        if let Some(idx) = data_index {
            self.data_mask(idx)
        } else {
            BitBlock::zero()
        }
    }

    #[inline]
    fn data_blocks_size_hint(&self) -> crate::ops::SizeHint {
        let len = self.data_blocks_len;
        (len, len)
    }
}

impl<Conf: Config, Data: DirectDataSource, const ALIGNED: bool> LevelMasksIterExt for DirectBitset<Conf, Data, ALIGNED>{
    type IterState = ();
    fn make_iter_state(&self) -> Self::IterState {()}
    unsafe fn drop_iter_state(&self, _: &mut std::mem::ManuallyDrop<Self::IterState>) {}

    type Level1BlockData = (Option<NonNull<Self>>, usize/*lvl1_block_index*/);

    #[inline]
    unsafe fn init_level1_block_data(
        &self,
        _: &mut Self::IterState,
        level1_block_data: &mut MaybeUninit<Self::Level1BlockData>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        if let Some(block_index) = self.lvl0_get_item(level0_index){
            level1_block_data.write((Some(self.into()), block_index));
            let mask = *self.lvl1_masks.get_unchecked(block_index);
            (mask, true)
        } else {
            level1_block_data.write((None, 0));    // TODO: Can we reach data after this?
            (BitBlock::zero(), false)
        }
    }

    #[inline]
    unsafe fn data_mask_from_block_data(
        level1_block_data: &Self::Level1BlockData,
        level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock {
        // TODO: Can this actually happens?
        if level1_block_data.0 == None {
            return BitBlock::zero();
        }

        let this = level1_block_data.0.unwrap_unchecked().as_ref();
        let lvl1_block_index = level1_block_data.1;

        let data_index = this.lvl1_get_item(lvl1_block_index, level1_index);
        if let Some(idx) = data_index {
            this.data_mask(idx)
        } else {
            BitBlock::zero()
        }
    }
}

impl_bitset!(
    impl<Conf, Data> const<ALIGNED: bool> for ref DirectBitset<Conf, Data, ALIGNED>
    where
        Conf: Config, Data: DirectDataSource
);

#[cfg(test)]
mod tests{
    use itertools::assert_equal;

use super::*;
    use crate::BitSet;

    // Mmap not supported by miri.
    #[cfg(not(miri))]
    #[test]
    fn mmap_test(){
        use memmap2::Mmap;

        type MmapBitset<Conf> = DirectBitset<Conf, Arc<Mmap>>;

        type Config = crate::config::_64bit;
        let mut file = tempfile::tempfile().unwrap();
        let etalon: BitSet<Config> = [1,2,3,4,66,100, 16089].into();
        etalon.serialize(&mut file).unwrap();

        let mmap = unsafe { Mmap::map(&file).unwrap()  };

        let b: MmapBitset<Config> = DirectBitset::new(Arc::new(mmap), 0).unwrap();

        for i in &etalon{
            assert!( b.contains(i) );
        }

        unsafe{
            assert_eq!(
                etalon.data_mask(0, 1),
                LevelMasks::data_mask(&b, 0, 1)
            );
        }
    }

    #[test]
    fn aligned_test(){
        use aligned_vec::{AVec, ConstAlign};

        type Conf = crate::config::_64bit;
        const ALIGN: usize = <Conf as Config>::MAX_MASK_ALIGN;
        type AlignedVec = AVec<u8, ConstAlign<ALIGN>>;

        let etalon: BitSet<Conf> = [1,2,3,4,66,100, 16089].into();
        let mut vec = Vec::new();
        etalon.serialize(&mut vec).unwrap();
        let avec = AlignedVec::from_slice(ALIGN, &vec);

        let im = DirectBitset::<Conf, &[u8], true>::new(&avec, 0).unwrap();

        assert_equal(etalon.iter(), im.iter());
    }
}
