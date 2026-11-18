use std::io::{Read, Write};
use std::mem::MaybeUninit;
use std::ops::{ControlFlow, Sub};
use crate::{BitBlock, BitSet, make_lvl0_u64_index_starts};
use crate::bitset::{Level0Block, Level1Block, LevelDataBlock};
use crate::bitset::block::Block;
use crate::config::Config;
use crate::impl_bitset::Primitive;
use crate::iter::BlockIter;
use crate::bitset::level::{IBlock, Level};
use crate::primitive_array::PrimitiveArray;
use crate::serialization::*;
use crate::config::*;

#[inline]
fn make_hierarchy_block<Mask, BlockIndices>(mask: Mask, index_offset: &mut BlockIndices::Item)
    -> Block<Mask, BlockIndices>
where
    Mask: BitBlock,
    BlockIndices: PrimitiveArray
{
    let block_indices = {
        let mut block_indices: BlockIndices = unsafe{MaybeUninit::zeroed().assume_init()};
        mask.for_each_bit(|i|{
            block_indices.as_mut()[i] = *index_offset;
            // Allowed to overflow here, on the very last round with
            // BlockIndices::Item=u8 and 256bit config.
            // (root level with 256 items)
            *index_offset = index_offset.wrapping_add(Primitive::ONE);
        });
        block_indices
    };
    unsafe{Block::from_parts(mask, block_indices)}
}

impl<Conf: Config> BitSet<Conf> {
    #[inline]
    fn traverse_lvl1_masks<F, B>(&self, mut f: F) -> ControlFlow<B>
    where
        F: FnMut(&Lvl1Mask<Conf>) -> ControlFlow<B>
    {
        self.level0.mask().traverse_bits(|i| -> ControlFlow<B> {
            let lvl1_block_index = unsafe{ self.level0.get_or_zero(i).as_usize() };
            let lvl1_block = unsafe{ self.level1.blocks().get_unchecked(lvl1_block_index) };
            f(lvl1_block.mask())
        })
    }

    /// Serialize container to a binary format.
    pub fn serialize(&self, w: &mut impl Write) -> std::io::Result<()> {
        let mut w = Writer::new(w);

        // version|lvl1_len|data_len||
        w.write_primitive::<u16>(SERIALIZATION_FORMAT_VER)?;
        w.write_primitive::<u16>(self.level1.len().sub(1) as _)?;
        w.write_primitive::<u32>(self.data.len().sub(1) as _)?;
        w.write_padding_for::<Lvl0Mask<Conf>>()?;

        // lvl0_mask
        let lvl0_mask = *self.level0.mask();
        w.write_mask(&lvl0_mask)?;
        w.write_padding_for::<Lvl0Index<Conf>>()?;

        // [lvl0_bitcount]
        let (lvl0_bitcounts, _) = make_lvl0_u64_index_starts::<Conf>(&lvl0_mask);
        w.write_primitives(&lvl0_bitcounts)?;
        w.write_padding_for::<Lvl1Mask<Conf>>()?;

        // [lvl1_mask]
        {
            let mut buf_w = BufferedWriter::new(&mut w);
            let ctrl = self.traverse_lvl1_masks(|&lvl1_mask| -> ControlFlow<_> {
                let res = buf_w.write_mask(lvl1_mask);
                match res {
                    Ok(_) => ControlFlow::Continue(()),
                    Err(e) => ControlFlow::Break(e)
                }
            });
            if let Some(e) = ctrl.break_value() {
                return Err(e);
            }
            buf_w.close()?;
            w.write_padding_for::<Lvl1Index<Conf>>()?;
        }

        // [lvl1_bitcount]
        {
            let mut bits_count = 0;
            let mut buf_w = BufferedWriter::new(&mut w);
            let ctrl = self.traverse_lvl1_masks(|lvl1_mask| -> ControlFlow<_> {
                for submask in lvl1_mask.as_array(){
                    let res = buf_w.write_primitive(Lvl1Index::<Conf>::from_usize(bits_count));
                    if let Err(e) = res {
                        return ControlFlow::Break(e);
                    }
                    bits_count += submask.count_ones();
                }
                ControlFlow::Continue(())
            });
            if let Some(e) = ctrl.break_value() {
                return Err(e);
            }
            buf_w.close()?;
            w.write_padding_for::<DataMask<Conf>>()?;
        }

        // [data]
        {
            let mut buf_w = BufferedWriter::new(&mut w);
            let ctrl = BlockIter::new(self).traverse(|block| -> ControlFlow<_> {
                let res = buf_w.write_mask(block.bit_block);
                match res {
                    Ok(_) => ControlFlow::Continue(()),
                    Err(e) => ControlFlow::Break(e)
                }
            });
            if let Some(e) = ctrl.break_value() {
                return Err(e);
            }
            buf_w.close()?;
        }

        Ok(())
    }

    /// Deserialize from [serialized](Self::serialize) BitSet.
    pub fn deserialize(r: &mut impl Read) -> Result<Self, AccessError> {
        let mut r = Reader::new(r);

        // version|lvl1_len|data_len||
        r.read_version()?;
        let lvl1_len = r.read_primitive::<u16>()? as usize;
        let data_len = r.read_primitive::<u32>()? as usize;
        r.read_padding_for::<Lvl0Mask<Conf>>()?;

        // lvl0_mask
        let level0: Level0Block<Conf> = {
            let mask = r.read_mask()?;
            let mut index_offset = Primitive::ONE;  // skip one for empty lvl1 block
            make_hierarchy_block(mask, &mut index_offset)
        };
        r.read_padding_for::<Lvl0Index<Conf>>()?;

        // [lvl0_bitcount]
        r.read_primitives(&mut [Lvl0Index::<Conf>::ZERO; 8])?;  // skip section
        r.read_padding_for::<Lvl1Mask<Conf>>()?;

        // [lvl1_mask]
        let level1 = {
            let mut blocks = Vec::with_capacity(lvl1_len+1);

            // Insert empty lvl1 block
            unsafe{ blocks.spare_capacity_mut().get_unchecked_mut(0).write(Default::default()); }
            let mut blocks_len = 1;

            let mut data_block_index_offset = Primitive::ONE;  // skip one for empty data block

            r.foreach_read_mask(lvl1_len, |&mask: &Conf::Level1BitBlock| {
                let block: Level1Block<Conf> = make_hierarchy_block(mask, &mut data_block_index_offset);
                unsafe{ blocks.spare_capacity_mut().get_unchecked_mut(blocks_len).write(block); }
                blocks_len +=1;
            })?;
            unsafe{ blocks.set_len(blocks_len); }

            unsafe{ Level::from_blocks_unchecked(blocks) }
        };
        r.read_padding_for::<Lvl1Index<Conf>>()?;

        // [lvl1_bitcount]
        r.skip_n::<Lvl1Index<Conf>>(lvl1_len * (Lvl1Mask::<Conf>::size() / 64))?;
        r.read_padding_for::<DataMask<Conf>>()?;

        // [data]
        let data = {
            let mut blocks: Vec<LevelDataBlock<Conf>> = Vec::with_capacity(data_len+1);

            // insert empty DataBlock
            unsafe{ blocks.spare_capacity_mut().get_unchecked_mut(0).write(Default::default()); }
            let mut blocks_len = 1;

            // This will copy as is directly for little endian in one go.
            {
                let slice = unsafe{std::slice::from_raw_parts_mut(
                    blocks.as_mut_ptr().add(1).cast::<DataMask<Conf>>(),
                    data_len
                )};
                r.read_masks(slice)?;
                blocks_len += data_len;
            }
            unsafe{ blocks.set_len(blocks_len); }

            unsafe{ Level::from_blocks_unchecked(blocks) }
        };

        Ok(Self{
            level0, level1, data,
        })
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use std::io::Cursor;
    use itertools::assert_equal;
    use crate::{DirectBitset, ImmutableBitset, config};
    use super::*;

    #[test]
    fn simple_serialize_test(){
        let mut bitset: BitSet<config::_64bit> = Default::default();
        bitset.insert(100);
        bitset.insert(5720);
        bitset.insert(219347);

        let im = ImmutableBitset::<config::_64bit>::from(&bitset);
        let mut etalon_serialization = Vec::new();
        im.serialize(&mut etalon_serialization).unwrap();

        let mut vec: Vec<u8> = Vec::new();
        bitset.serialize(&mut vec).unwrap();
        println!("Orig {:?}", bitset);
        println!("Serialized {:?}", vec);

        assert_eq!(&vec, &etalon_serialization);

        let deserialized_bitset: BitSet<config::_64bit> = BitSet::deserialize(&mut Cursor::new(vec)).unwrap();
        println!("Deserialized {:?}", deserialized_bitset);

        assert_eq!(bitset, deserialized_bitset);
        assert_equal(bitset.iter(), deserialized_bitset.iter());    // check by iter too.
    }

    /// Fills all but 3 level1 blocks, with 1 to 4 data blocks each, so that
    /// the level1 and data sections span several 64-item chunks of the
    /// big-endian reader/writer plus a remainder, and for `_256bit` the
    /// level1 bitcounts span the 512-item chunks too. Small enough for Miri.
    fn chunked_serialize_test<Conf: Config>(){
        let lvl0_bits = Lvl0Mask::<Conf>::size();
        let lvl1_bits = Lvl1Mask::<Conf>::size();
        let data_bits = DataMask::<Conf>::size();

        let mut bitset: BitSet<Conf> = Default::default();
        for lvl1 in 0..lvl0_bits - 3 {
            for data in 0..lvl1 % 4 + 1 {
                let start = (lvl1 * lvl1_bits + data * 3) * data_bits;
                for bit in [0, 1 + (lvl1 + data) % (data_bits - 2), data_bits - 1] {
                    bitset.insert(start + bit);
                }
            }
        }

        let mut vec: Vec<u8> = Vec::new();
        bitset.serialize(&mut vec).unwrap();

        // BitSet and ImmutableBitset go through different writers.
        let im = ImmutableBitset::<Conf>::from(&bitset);
        let mut im_vec: Vec<u8> = Vec::new();
        im.serialize(&mut im_vec).unwrap();
        assert_eq!(vec, im_vec);

        // The data section ends the stream: every u64 lane little-endian,
        // lanes in order. This is built without the code under test, so a
        // byte order mistake made the same way on write and read is caught.
        let data_le: Vec<u8> = bitset.block_iter()
            .flat_map(|block| block.bit_block.as_array().to_vec())
            .flat_map(u64::to_le_bytes)
            .collect();
        assert!(vec.ends_with(&data_le));

        let deserialized: BitSet<Conf> = BitSet::deserialize(&mut vec.as_slice()).unwrap();
        assert_eq!(bitset, deserialized);
        assert_equal(bitset.iter(), deserialized.iter());
        let mut re_vec: Vec<u8> = Vec::new();
        deserialized.serialize(&mut re_vec).unwrap();
        assert_eq!(vec, re_vec);

        let im = ImmutableBitset::<Conf>::deserialize(&mut vec.as_slice()).unwrap();
        assert_equal(bitset.iter(), im.iter());
        let mut re_vec: Vec<u8> = Vec::new();
        im.serialize(&mut re_vec).unwrap();
        assert_eq!(vec, re_vec);

        let direct = DirectBitset::<Conf, &[u8]>::new(vec.as_slice(), 0).unwrap();
        assert_equal(bitset.iter(), direct.iter());
        for i in bitset.iter(){
            assert!(im.contains(i));
            assert!(direct.contains(i));
        }
    }

    #[test]
    fn chunked_serialize_64_test(){
        chunked_serialize_test::<config::_64bit>();
    }

    #[cfg(feature = "simd")]
    #[test]
    fn chunked_serialize_128_test(){
        chunked_serialize_test::<config::_128bit>();
    }

    #[cfg(feature = "simd")]
    #[test]
    fn chunked_serialize_256_test(){
        chunked_serialize_test::<config::_256bit>();
    }
}
