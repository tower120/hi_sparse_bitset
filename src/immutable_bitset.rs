mod serialization;

use core::slice;
use std::{
    mem::{ManuallyDrop, MaybeUninit},
    ptr::NonNull
};
use crate::{
    BitBlock,
    BitSetBase,
    BitSetInterface,
    config::*,
    impl_bitset::{LevelMasks, LevelMasksIterExt, impl_bitset},
    primitive::Primitive,
};

/// Bitset with serialized-like linear data structure.
///
/// This is the fastest structure to materialize, deserialize and serialize.
pub struct ImmutableBitset<Conf: Config>{
    lvl0_mask: Lvl0Mask<Conf>,
    lvl0_u64_index_starts: [Lvl0Index<Conf>; 8],

    lvl1_masks: Vec<Lvl1Mask<Conf>>,
    lvl1_u64_index_starts: Vec<Lvl1Index<Conf>>,

    data: Vec<DataMask<Conf>>
}

/* /// Reusable blank for [ImmutableBitset] construction.
pub struct ImmutableBitsetBlank<Conf: Config>(ImmutableBitset<Conf>);

impl ImmutableBitsetBlank<Conf: Config>{
    pub fn materialize(self, bitset: impl BitSetInterface) -> ImmutableBitset;
    pub fn deserialize(self) -> ImmutableBitset;
} */

#[inline(always)]
unsafe fn push_within_capacity<T>(v: &mut Vec<T>, item: T){
    v.spare_capacity_mut().first_mut().unwrap_unchecked().write(item);
    v.set_len(v.len()+1);
 }

#[inline]
fn masks_as_u64<Mask: BitBlock>(slice: &[Mask]) -> &[u64]{
    unsafe {
        slice::from_raw_parts(
            slice.as_ptr().cast(),
            slice.len() * (size_of::<Mask>() / 8)
        )
    }
}

#[inline]
fn make_lvl0_u64_index_starts<Conf: Config>(lvl0_mask: &Lvl0Mask<Conf>)
    -> ([Lvl0Index<Conf>; 8], usize/*total risen bits count*/)
{
    let mut bits_count = 0;
    let mut lvl0_u64_index_starts = [Primitive::ZERO; 8];
    for (idx, sub_mask) in lvl0_mask.as_array().iter().enumerate(){
        unsafe{
            *lvl0_u64_index_starts.get_unchecked_mut(idx) =
                Primitive::from_usize(bits_count);
        };
        bits_count += sub_mask.count_ones();
    }
    (lvl0_u64_index_starts, bits_count)
}

#[inline]
fn fill_lvl1_u64_index_starts<Conf: Config>(
    lvl1_masks: &[Lvl1Mask<Conf>],
    vec: &mut Vec<Lvl1Index<Conf>>
) -> usize /*total risen bits count*/{
    let sub_masks = masks_as_u64(lvl1_masks);
    let len = sub_masks.len();
    vec.reserve_exact(len);

    let mut bits_count = 0;
    for idx in 0..len{
        unsafe{
            let sub_mask = sub_masks.get_unchecked(idx);

            *vec.spare_capacity_mut().get_unchecked_mut(idx) =
                MaybeUninit::new(Primitive::from_usize(bits_count));

            bits_count += sub_mask.count_ones();
        }
    }
    unsafe{ vec.set_len(len); }

    bits_count
}

#[inline(always)]
fn lvl_get_item<LvlMask:BitBlock>(
    offsets: &[impl Primitive],
    sub_masks: &[u64],
    sub_mask_index_offset: usize,
    index: usize
) -> Option<usize> {
    use crate::bit_utils::*;

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

impl<Conf: Config> ImmutableBitset<Conf>{
    #[inline]
    fn new() -> Self{
        Self{
            lvl0_mask: BitBlock::zero(),
            lvl0_u64_index_starts: Default::default(),
            lvl1_masks: Vec::new(),
            lvl1_u64_index_starts: Vec::new(),
            data: Vec::new()
        }
    }

    #[inline]
    fn lvl0_get_item(&self, index: usize) -> Option<usize> {
        lvl_get_item::<Lvl0Mask<Conf>>(
            &self.lvl0_u64_index_starts,
            self.lvl0_mask.as_array(),
            0,
            index
        )
    }

    #[inline]
    fn lvl1_get_item(&self, lvl1_block_index: usize, level1_index: usize) -> Option<usize> {
        lvl_get_item::<Lvl1Mask<Conf>>(
            &self.lvl1_u64_index_starts,
            masks_as_u64(&self.lvl1_masks),
            lvl1_block_index * (size_of::<Lvl1Mask<Conf>>() / 8),
            level1_index
        )
    }
}

impl<Conf: Config, Other: BitSetInterface<Conf=Conf>> From<Other> for ImmutableBitset<Conf>{
    fn from(other: Other) -> Self {
        let mut lvl0_mask: Lvl0Mask<Conf> = BitBlock::zero();
        let mut lvl1_masks = Vec::new();
        let mut data = Vec::new();

        let other_level0_mask = other.level0_mask();
        if Other::TRUSTED_HIERARCHY {
            lvl0_mask = other_level0_mask;
            lvl1_masks.reserve_exact(lvl0_mask.count_ones());
        }

        let data_size_hint = other.data_blocks_size_hint();
        if data_size_hint.0 == data_size_hint.1 {
            // We want ImmutableBitset as lean as possible.
            data.reserve_exact(data_size_hint.0)
        } else {
            data.reserve(data_size_hint.0)
        }

        let mut other_iter_state = other.make_iter_state();

        // Traverse Lvl0
        other_level0_mask.for_each_bit(|lvl0_idx|{
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

            let mut lvl1_mask: Lvl1Mask<Conf> = BitBlock::zero();
            if Other::TRUSTED_HIERARCHY{
                lvl1_mask = other_lvl1_mask;
                data.reserve(lvl1_mask.count_ones());
                unsafe{ push_within_capacity(&mut lvl1_masks, lvl1_mask) }
            }

            // Traverse Lvl1
            other_lvl1_mask.for_each_bit(|lvl1_idx|{
                let other_data = unsafe{
                    Other::data_mask_from_block_data(
                        &mut other_level1_block_data,
                        lvl1_idx
                    )
                };

                if Other::TRUSTED_HIERARCHY{
                    unsafe{ push_within_capacity(&mut data, other_data) }
                } else {
                    if !other_data.is_zero(){
                        data.push(other_data);
                        unsafe{ lvl1_mask.set_bit_unchecked::<true>(lvl1_idx); }
                    }
                }
            });

            if !Other::TRUSTED_HIERARCHY    // we already formed and pushed masks.
            && !lvl1_mask.is_zero() {
                lvl1_masks.push(lvl1_mask);
                unsafe{ lvl0_mask.set_bit_unchecked::<true>(lvl0_idx); }
            }
        });
        unsafe{ other.drop_iter_state(&mut ManuallyDrop::new(other_iter_state)); }

        let (lvl0_u64_index_starts, _) = make_lvl0_u64_index_starts::<Conf>(&lvl0_mask);

        let mut lvl1_u64_index_starts = Vec::new();
        fill_lvl1_u64_index_starts::<Conf>(&lvl1_masks, &mut lvl1_u64_index_starts);

        Self{
            lvl0_mask,
            lvl0_u64_index_starts,
            lvl1_masks,
            lvl1_u64_index_starts,
            data,
        }
    }
}

impl<Conf: Config> BitSetBase for ImmutableBitset<Conf>{
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = true;
}

impl<Conf: Config> LevelMasks for ImmutableBitset<Conf>{
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
            unsafe{ *self.data.get_unchecked(idx) }
        } else {
            BitBlock::zero()
        }
    }

    #[inline]
    fn data_blocks_size_hint(&self) -> crate::ops::SizeHint {
        let len = self.data.len();
        (len, len)
    }
}

impl<Conf: Config> LevelMasksIterExt for ImmutableBitset<Conf>{
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
            *this.data.get_unchecked(idx)
        } else {
            BitBlock::zero()
        }
    }
}

impl_bitset!(impl<Conf> for ref ImmutableBitset<Conf> where Conf: Config);

#[cfg(test)]
mod tests{
    use itertools::assert_equal;
    use crate::{BitSet, config};
    use super::*;

    #[test]
    fn materialize_test(){
        type Conf = config::_64bit;
        let bitset: BitSet<Conf> = [1,2,3,500, 12836].into();

        let im: ImmutableBitset<Conf> = (&bitset).into();

        assert_equal(&bitset,&im);
    }

}