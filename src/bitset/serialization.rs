use std::io::{Read, Write};
use std::mem::MaybeUninit;
use std::ops::ControlFlow;
use std::{cmp, mem, slice};
use crate::{BitBlock, BitSet};
use crate::bitset::{Level0Block, Level1Block, LevelDataBlock};
use crate::bitset::block::Block;
use crate::config::Config;
use crate::internals::Primitive;
use crate::iter::BlockIter;
use crate::bitset::level::{IBlock, Level};
use crate::primitive_array::PrimitiveArray;

#[inline]
fn read_mask<Mask: BitBlock>(r: &mut impl Read) -> std::io::Result<Mask> {
    let mut buf = Mask::zero().to_ne_bytes();
    r.read_exact(buf.as_mut())?;
    Ok(Mask::from_le_bytes(buf))
}

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

#[inline]
fn get_padding(pos: usize, align: usize) -> usize {
    (pos as *const u8).align_offset(align)
}

#[inline]
fn write_padding(w: &mut impl Write, padding: usize) -> std::io::Result<()> {
    const BUFFER: [u8; 1024] = [0; 1024];
    w.write_all(&BUFFER[..padding])
}

#[inline]
fn read_padding(r: &mut impl Read, mut padding: usize) -> std::io::Result<()> {
    const BUFFER_SIZE: usize = 64;
    let mut buffer: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
    while padding != 0 {
        let slice = unsafe{
            slice::from_raw_parts_mut(
                buffer.as_mut_ptr(),
                cmp::min(padding, BUFFER_SIZE),
            )
        };

        padding -= r.read(slice)?;
    }
    Ok(())
}

#[inline]
pub(crate) fn lvl0_padding<Conf: Config>(pos: usize) -> usize {
    get_padding(pos, align_of::<<Conf as Config>::Level1BitBlock>())
}

#[inline]
pub(crate) fn lvl1_padding<Conf: Config>(pos: usize) -> usize {
    get_padding(pos, align_of::<<Conf as Config>::DataBitBlock>())
}

impl<Conf: Config> BitSet<Conf> {
    /// Serialize container to a binary format.
    ///
    /// # Format
    ///
    /// In little endian.
    /// ```text
    /// lvl0_mask|lvl0_padding|[lvl1_mask;..]|lvl1_padding|[data;..]
    /// ```
    /// Where
    /// * `lvl0_padding` - is padding needed to for the first lvl1_mask to be aligned.
    /// * `lvl1_padding` - is padding needed to for the first data to be aligned.
    /// If all masks are the same size - there will be no padding.
    pub fn serialize(&self, w: &mut impl Write) -> std::io::Result<()> {
        // lvl0_mask
        let lvl0_mask = self.level0.mask();
        let lvl0_mask_bytes = lvl0_mask.to_le_bytes();
        w.write_all(lvl0_mask_bytes.as_ref())?;

        let mut pos = size_of::<<Conf as Config>::Level0BitBlock>();

        // lvl0_padding
        let lvl0_padding = lvl0_padding::<Conf>(pos);
        write_padding(w, lvl0_padding)?;
        pos += lvl0_padding;

        // [lvl1_mask;..]
        let ctrl = lvl0_mask.traverse_bits(|i| -> ControlFlow<_> {
            let lvl1_block_index = unsafe{ self.level0.get_or_zero(i).as_usize() };
            let lvl1_block = unsafe{ self.level1.blocks().get_unchecked(lvl1_block_index) };

            let res = w.write_all(lvl1_block.mask().to_le_bytes().as_ref());
            match res {
                Ok(_) => ControlFlow::Continue(()),
                Err(e) => ControlFlow::Break(e)
            }
        });
        if let Some(e) = ctrl.break_value() {
            return Err(e);
        }
        pos += lvl0_mask.count_ones() * size_of::<<Conf as Config>::Level1BitBlock>();

        // lvl1_padding
        let lvl1_padding = lvl1_padding::<Conf>(pos);
        write_padding(w, lvl1_padding)?;

        // [data;..]
        let ctrl = BlockIter::new(self).traverse(|block| -> ControlFlow<_> {
            let res = w.write_all(block.bit_block.to_le_bytes().as_ref());
            match res {
                Ok(_) => ControlFlow::Continue(()),
                Err(e) => ControlFlow::Break(e)
            }
        });
        if let Some(e) = ctrl.break_value() {
            return Err(e);
        }

        Ok(())
    }

    /// Deserialize from [serialized](Self::serialize) BitSet.
    pub fn deserialize(r: &mut impl Read) -> std::io::Result<Self> {
        const BUF_SIZE: usize = 32;

        #[inline]
        fn chunked<E, F>(len: usize, chunk_size: usize, mut f: F)
            -> F::Output
        where
            F: FnMut(usize) -> Result<(), E>
        {
            for _ in 0..len/chunk_size {
                f(chunk_size)?;
            }
            let rem = len % chunk_size;
            if rem != 0{
                f(rem)?;
            }
            Ok(())
        }

        #[inline]
        fn chunked_read<F, T>(read: &mut impl Read, len: usize, mut f: F)
            -> std::io::Result<()>
        where
            F: FnMut(&[T])
        {
            let mut buf: [MaybeUninit<T>; BUF_SIZE] = unsafe{MaybeUninit::uninit().assume_init()};
            chunked(len, BUF_SIZE, |size: usize| -> std::io::Result<()> {
                let bytes = unsafe{ slice::from_raw_parts_mut(buf.as_mut_ptr().cast(), size * size_of::<T>()) };
                read.read_exact(bytes)?;
                // mem::transmute for array_assume_init
                let slice = unsafe{mem::transmute(buf.get_unchecked(..size))};
                f(slice);
                Ok(())
            })
        }

        // Level 0
        let level0: Level0Block<Conf> = {
            let mask = read_mask(r)?;
            let mut index_offset = Primitive::ONE;  // skip one for empty lvl1 block
            make_hierarchy_block(mask, &mut index_offset)
        };
        let mut pos = size_of::<<Conf as Config>::Level0BitBlock>();

        // lvl0_padding
        let lvl0_padding = lvl0_padding::<Conf>(pos);
        read_padding(r, lvl0_padding)?;
        pos += lvl0_padding;

        // Level 1
        let lvl1_len = level0.mask().count_ones();
        let (level1, data_blocks_len) = {
            let mut blocks = Vec::with_capacity(lvl1_len+1);

            // Insert empty lvl1 block
            unsafe{ blocks.spare_capacity_mut().get_unchecked_mut(0).write(Default::default()); }
            let mut blocks_len = 1;

            let mut data_block_index_offset = Primitive::ONE;  // skip one for empty data block

            chunked_read(r, lvl1_len, |masks: &[Conf::Level1BitBlock]| {
                for mask in masks {
                    let mask = (*mask).to_le();
                    let block: Level1Block<Conf> = make_hierarchy_block(mask, &mut data_block_index_offset);

                    unsafe{ blocks.spare_capacity_mut().get_unchecked_mut(blocks_len).write(block); }
                    blocks_len +=1;
                }
            })?;
            unsafe{ blocks.set_len(blocks_len); }

            (
                unsafe{ Level::from_blocks_unchecked(blocks) },
                data_block_index_offset.as_usize()-1
            )
        };
        pos += lvl1_len * size_of::<<Conf as Config>::Level1BitBlock>();

        // lvl1_padding
        let lvl1_padding = lvl1_padding::<Conf>(pos);
        read_padding(r, lvl1_padding)?;

        // Data level
        let data = {
            let mut blocks: Vec<LevelDataBlock<Conf>> = Vec::with_capacity(data_blocks_len+1);

            // insert empty DataBlock
            unsafe{ blocks.spare_capacity_mut().get_unchecked_mut(0).write(Default::default()); }
            let mut blocks_len = 1;

            #[cfg(target_endian = "little")]
            {
                // Copy as is directly for little endian in one go.
                let bytes = unsafe{slice::from_raw_parts_mut(
                    blocks.as_mut_ptr().add(1).cast::<u8>(),
                    data_blocks_len * size_of::<Conf::DataBitBlock>()
                )};
                r.read_exact(bytes)?;
                blocks_len += data_blocks_len;
            }

            #[cfg(target_endian = "big")]
            chunked_read(r, data_blocks_len, |masks: &[Conf::DataBitBlock]| {
                 for mask in masks {
                     let mask = (*mask).to_le();
                     let block: LevelDataBlock<Conf> = unsafe{Block::from_parts(
                         mask, []
                     )};

                     unsafe{ blocks.spare_capacity_mut().get_unchecked_mut(blocks_len).write(block); }
                    blocks_len +=1;
                 }
            })?;
            unsafe{ blocks.set_len(blocks_len); }

            unsafe{ Level::from_blocks_unchecked(blocks) }
        };

        Ok(Self{
            level0, level1, data,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use itertools::assert_equal;
    use crate::config;
    use super::*;

    #[test]
    fn simple_serialize_test(){
        let mut bitset: BitSet<config::_64bit> = Default::default();
        bitset.insert(100);
        bitset.insert(5720);
        bitset.insert(219347);

        let mut vec: Vec<u8> = Vec::new();
        bitset.serialize(&mut vec).unwrap();
        println!("Orig {:?}", bitset);
        println!("Serialized {:?}", vec);

        let deserialized_bitset: BitSet<config::_64bit> = BitSet::deserialize(&mut Cursor::new(vec)).unwrap();
        println!("Deserialized {:?}", deserialized_bitset);

        assert_eq!(bitset, deserialized_bitset);
        assert_equal(bitset.iter(), deserialized_bitset.iter());    // check by iter too.
    }
}