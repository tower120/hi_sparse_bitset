use std::io::{Read, Write};

use crate::serialization::*;

use super::*;

impl<Conf: Config> ImmutableBitset<Conf>{
    /// Serialize container to a binary format.
    pub fn serialize(&self, w: &mut impl Write) -> std::io::Result<()> {
        let mut w = Writer::new(w);

        // version|lvl1_len|data_len||
        w.write_primitive::<u16>(SERIALIZATION_FORMAT_VER)?;
        w.write_primitive::<u16>(self.lvl1_masks.len() as _)?;
        w.write_primitive::<u32>(self.data.len() as _)?;
        w.write_padding_for::<Lvl0Mask<Conf>>()?;

        // lvl0_mask
        w.write_mask(&self.lvl0_mask)?;
        w.write_padding_for::<Lvl0Index<Conf>>()?;

        // [lvl0_bitcount]
        w.write_primitives(&self.lvl0_u64_index_starts)?;
        w.write_padding_for::<Lvl1Mask<Conf>>()?;

        // [lvl1_mask]
        w.write_masks(&self.lvl1_masks)?;
        w.write_padding_for::<Lvl1Index<Conf>>()?;

        // [lvl1_bitcount]
        w.write_primitives(&self.lvl1_u64_index_starts)?;
        w.write_padding_for::<DataMask<Conf>>()?;

        // [data]
        w.write_masks(&self.data)?;

        Ok(())
    }

    fn deserialize_impl(&mut self, r: &mut impl Read) -> Result<(), AccessError> {
        let mut r = Reader::new(r);

        // version|lvl1_len|data_len||
        r.read_version()?;
        let lvl1_len = r.read_primitive::<u16>()? as usize;
        let data_len = r.read_primitive::<u32>()? as usize;
        r.read_padding_for::<Lvl0Mask<Conf>>()?;

        // lvl0_mask
        self.lvl0_mask = r.read_mask()?;
        r.read_padding_for::<Lvl0Index<Conf>>()?;

        // [lvl0_bitcount]
        r.read_primitives(&mut self.lvl0_u64_index_starts)?;
        r.read_padding_for::<Lvl1Mask<Conf>>()?;

        // [lvl1_mask]
        debug_assert!(self.lvl1_masks.is_empty());
        r.read_masks_to_vec(&mut self.lvl1_masks, lvl1_len)?;
        r.read_padding_for::<Lvl1Index<Conf>>()?;

        // [lvl1_bitcount]
        debug_assert!(self.lvl1_u64_index_starts.is_empty());
        r.read_primitives_to_vec(
            &mut self.lvl1_u64_index_starts,
            lvl1_len * (Lvl1Mask::<Conf>::size() / 64)
        )?;
        r.read_padding_for::<DataMask<Conf>>()?;

        // [data]
        debug_assert!(self.data.is_empty());
        r.read_masks_to_vec(&mut self.data, data_len)?;

        Ok(())
    }

    pub fn deserialize(r: &mut impl Read) -> Result<Self, AccessError> {
        let mut this = Self::new();
        this.deserialize_impl(r)?;
        Ok(this)
    }
}

#[cfg(test)]
mod tests{
    use std::io::Cursor;
    use itertools::assert_equal;
    use crate::{BitSet, config};
    use super::*;

    #[test]
    fn serialization_test(){
        type Conf = config::_128bit<wide::u64x4>;
        let bitset: BitSet<Conf> = [1,2,3,4 /* 500, 12836, 123948 */].into();

        let im: ImmutableBitset<Conf> = (&bitset).into();
        assert_equal(&bitset,&im);

        let mut vec: Vec<u8> = Vec::new();
        im.serialize(&mut vec).unwrap();

        let im = ImmutableBitset::<Conf>::deserialize(&mut Cursor::new(&vec)).unwrap();
        for i in bitset.iter(){
            assert!(im.contains(i));
        }
        assert_equal(&bitset,&im);
    }
}
