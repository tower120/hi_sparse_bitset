use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Arc;

use crate::bit_utils::{get_bit_unchecked, zero_high_bits_unchecked};
use crate::impl_bitset::{LevelMasks, LevelMasksIterExt, impl_bitset};
use crate::primitive::Primitive;
use crate::{BitBlock, BitSetBase};
use crate::config::*;
use crate::serialization::*;

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

#[derive(Clone)]
pub struct DirectBitset<Conf: Config, Data, const ALIGNED: bool = false>{
    data_src: Data,

    base_offset: usize,
    lvl1_u64_bitcounts_offset: usize,
    data_offset: usize,

    phantom: PhantomData<Conf>
}

#[inline]
fn ptr_is_aligned_to<T>(ptr: *const T, align: usize) -> bool {
    if !align.is_power_of_two() {
        panic!("is_aligned_to: align is not a power-of-two");
    }

    ptr.addr() & (align - 1) == 0
}

#[inline]
unsafe fn read_header<const ALIGNED: bool>(ptr: *const u8) -> (u16, u16, u32) {
    let version : u16;
    let lvl1_len: u16;
    let data_len: u32;
    if ALIGNED{
        version  = ptr.cast::<u16>().read();
        lvl1_len = ptr.add(2).cast::<u16>().read();
        data_len = ptr.add(4).cast::<u32>().read();
    } else {
        version  = ptr.cast::<u16>().read_unaligned();
        lvl1_len = ptr.add(2).cast::<u16>().read_unaligned();
        data_len = ptr.add(4).cast::<u32>().read_unaligned();
    }
    (
        u16::from_le(version),
        u16::from_le(lvl1_len),
        u32::from_le(data_len),
    )
}

impl<Conf: Config, Data: DirectDataSource, const ALIGNED: bool> DirectBitset<Conf, Data, ALIGNED> {
    /// * `data` - data source that points to byte data.
    /// * `offset` - `data` offset in bytes, where serialized data begins.
    ///
    /// For `ALIGNED`, DirectBitset `data` + `offset` must be aligned to MAX_MASK_ALIGN,
    /// otherwise error will be returned.
    pub fn new(data: Data, offset: usize) -> Result<Self, AccessError> {
        let slice = &data.data_src()[offset..];
        let ptr = slice.as_ptr();
        let len = slice.len();

        if ALIGNED {
            let aligned = ptr_is_aligned_to(ptr, Conf::MAX_MASK_ALIGN);
            if !aligned{
                return Err(AccessError::Unaligned(Conf::MAX_MASK_ALIGN));
            }
        }

        let (version, lvl1_len, data_len) = unsafe{ read_header::<ALIGNED>(ptr) };
        check_version(version)?;
        let lvl1_len = lvl1_len as usize;
        let data_len = data_len as usize;

        let offsets = Offsets::<Conf>::new(lvl1_len);
        if len < offsets.len(data_len){
            use std::io::*;
            return Err(
                AccessError::IOError(
                    Error::from(ErrorKind::UnexpectedEof)
                )
            );
        }

        let data_offset = offsets.data_offset + offset;

        Ok(Self{
            data_src: data,
            base_offset: offset,
            lvl1_u64_bitcounts_offset: offsets.lvl1_bitcounts_offset + offset,
            data_offset,
            phantom: PhantomData,
        })
    }

    #[inline]
    fn lvl0_mask_ptr(&self) -> *const Lvl0Mask<Conf>{
        let ptr = self.data_src.data_src().as_ptr();
        unsafe{
            ptr.add(self.base_offset + Offsets::<Conf>::LVL0_MASK_OFFSET)
        }.cast()
    }

    #[inline]
    fn lvl0_u64_bitcounts(&self) -> *const Lvl0Index<Conf>{
        let ptr = self.data_src.data_src().as_ptr();
        unsafe{
            ptr.add(self.base_offset + Offsets::<Conf>::LVL0_BITCOUNTS_OFFSET)
        }.cast()
    }

    #[inline]
    fn lvl1_masks_ptr(&self) -> *const Lvl1Mask<Conf>{
        let ptr = self.data_src.data_src().as_ptr();
        unsafe{
            ptr.add(self.base_offset + Offsets::<Conf>::LVL1_MASKS_OFFSET)
        }.cast()
    }

    #[inline]
    fn lvl1_u64_bitcounts(&self) -> *const Lvl1Index<Conf>{
        let ptr = self.data_src.data_src().as_ptr();
        unsafe{
            ptr.add(self.lvl1_u64_bitcounts_offset)
        }.cast()
    }

    #[inline]
    fn data_masks_ptr(&self) -> *const DataMask<Conf>{
        let ptr = self.data_src.data_src().as_ptr();
        unsafe{
            ptr.add(self.data_offset)
        }.cast()
    }

    #[inline]
    unsafe fn read_primitive<T: Primitive>(ptr: *const T) -> T {
        let value = if ALIGNED{
            ptr.read()
        } else {
            ptr.read_unaligned()
        };

        #[cfg(target_endian = "little")]
        return value;

        #[cfg(target_endian = "big")]
        return value.swap_bytes();
    }

    #[inline]
    unsafe fn read_mask<Mask: BitBlock>(ptr: *const Mask) -> Mask {
        #[cfg(target_endian = "little")]
        {
            if ALIGNED{
                ptr.read()
            } else {
                ptr.read_unaligned()
            }
        }

        #[cfg(target_endian = "big")]
        {
            let mut bytes: MaybeUninit<Mask::BytesArray> = MaybeUninit::uninit();
            if ALIGNED{
                // cast to mask
                copy_nonoverlapping(
                    ptr,
                    bytes.as_mut_ptr().cast(),
                    size_of::<Mask>()
                );
            } else {
                // cast to bytes
                copy_nonoverlapping(
                    ptr.cast::<u8>(),
                    bytes.as_mut_ptr().cast::<u8>(),
                    size_of::<Mask>()
                );
            }
            Mask::from_le_bytes(bytes.assume_init())
        }
    }

    // TODO: For big endian, we need to take into account swapped sub-masks
    //       inside mask to correct index.
    #[cfg(target_endian = "little")]
    #[inline(always)]
    unsafe fn lvl_get_item<LvlMask:BitBlock>(
        offsets: *const impl Primitive,
        sub_masks: *const u64,
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

        let offset = Self::read_primitive(offsets.add(u64_index)).as_usize();
        let sub_mask = Self::read_mask(sub_masks.add(u64_index));
        if !get_bit_unchecked(sub_mask, bit_index) {
            return None;
        }
        Some(offset +
            zero_high_bits_unchecked(sub_mask, bit_index).count_ones() as usize
        )
    }

    #[inline]
    fn lvl0_get_item(&self, index: usize) -> Option<usize> {
        unsafe{
        Self::lvl_get_item::<Lvl0Mask<Conf>>(
            self.lvl0_u64_bitcounts(),
            self.lvl0_mask_ptr().cast::<u64>(),
            0,
            index
        )
        }
    }

    #[inline]
    fn lvl1_get_item(&self, lvl1_block_index: usize, level1_index: usize) -> Option<usize> {
        unsafe{
        Self::lvl_get_item::<Lvl1Mask<Conf>>(
            self.lvl1_u64_bitcounts(),
            self.lvl1_masks_ptr().cast::<u64>(),
            lvl1_block_index * (size_of::<Lvl1Mask<Conf>>() / 8),
            level1_index
        )
        }
    }
}

impl<Conf: Config, Data: DirectDataSource, const ALIGNED: bool> BitSetBase for DirectBitset<Conf, Data, ALIGNED>{
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = true;
}

impl<Conf: Config, Data: DirectDataSource, const ALIGNED: bool> LevelMasks for DirectBitset<Conf, Data, ALIGNED>{
    #[inline]
    fn level0_mask(&self) -> Lvl0Mask<Conf> {
        unsafe{ Self::read_mask(self.lvl0_mask_ptr()) }
    }

    #[inline]
    unsafe fn level1_mask(&self, level0_index: usize) -> Lvl1Mask<Conf> {
        if let Some(block_index) = self.lvl0_get_item(level0_index){
            Self::read_mask(
                self.lvl1_masks_ptr().add(block_index)
            )
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
            Self::read_mask(
                self.data_masks_ptr().add(idx)
            )
        } else {
            BitBlock::zero()
        }
    }

    #[inline]
    fn data_blocks_size_hint(&self) -> crate::ops::SizeHint {
        let len = unsafe{
            let ptr = self.data_src.data_src().as_ptr();
            read_header::<ALIGNED>(ptr.add(self.base_offset)).2
        } as usize;
        (len, len)
    }
}

impl<Conf: Config, Data: DirectDataSource, const ALIGNED: bool> LevelMasksIterExt for DirectBitset<Conf, Data, ALIGNED>{
    type IterState = ();
    type Level1BlockData = (Option<NonNull<Self>>, usize/*lvl1_block_index*/);

    #[inline]
    fn make_iter_state(&self) -> Self::IterState {()}

    #[inline]
    unsafe fn drop_iter_state(&self, _: &mut std::mem::ManuallyDrop<Self::IterState>) {}

    #[inline]
    unsafe fn init_level1_block_data(
        &self,
        _: &mut Self::IterState,
        level1_block_data: &mut MaybeUninit<Self::Level1BlockData>,
        level0_index: usize
    ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
        if let Some(block_index) = self.lvl0_get_item(level0_index){
            level1_block_data.write((Some(self.into()), block_index));
            let mask = Self::read_mask(
                self.lvl1_masks_ptr().add(block_index)
            );
            (mask, true)
        } else {
            level1_block_data.write((None, 0));    // TODO: Can we reach data after this?
            (BitBlock::zero(), false)
        }
    }

    #[inline]
    unsafe fn data_mask_from_block_data(
        level1_block_data: &Self::Level1BlockData, level1_index: usize
    ) -> <Self::Conf as Config>::DataBitBlock {
        // TODO: Can this actually happens?
        if level1_block_data.0 == None {
            return BitBlock::zero();
        }

        let this = level1_block_data.0.unwrap_unchecked().as_ref();
        let lvl1_block_index = level1_block_data.1;

        let data_index = this.lvl1_get_item(lvl1_block_index, level1_index);
        if let Some(idx) = data_index {
            Self::read_mask(
                this.data_masks_ptr().add(idx)
            )
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
    use crate::{BitSet, ImmutableBitset};

    #[test]
    fn aligned_test(){
        use aligned_vec::{AVec, ConstAlign};

        cfg_select! {
            miri => {
                type Conf = crate::config::_64bit;
                const SIZE: usize = 10_000;
            }
            _ => {
                type Conf = crate::config::_256bit;
                const SIZE: usize = 1_000_000;
            }
        }

        const ALIGN: usize = <Conf as Config>::MAX_MASK_ALIGN;
        type AlignedVec = AVec<u8, ConstAlign<ALIGN>>;

        let etalon: BitSet<Conf> = (0..SIZE).into_iter().collect();
        let etalon: ImmutableBitset<Conf> = (&etalon).into();

        let mut vec = Vec::new();
        etalon.serialize(&mut vec).unwrap();
        let avec = AlignedVec::from_slice(ALIGN, &vec);

        let im = DirectBitset::<Conf, &[u8], true>::new(&avec, 0).unwrap();
        for i in etalon.iter(){
            assert!(im.contains(i));
        }
        assert_equal(etalon.iter(), im.iter());
    }
}
