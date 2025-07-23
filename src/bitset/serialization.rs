use std::io::{Read, Write};
use std::mem::MaybeUninit;
use std::ops::ControlFlow;
use crate::{BitBlock, BitSet};
use crate::bitset::{Level0Block, Level1Block, LevelDataBlock, RawBitSet};
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
            *index_offset += Primitive::ONE;
        });
        block_indices
    };
    unsafe{Block::from_parts(mask, block_indices)}        
} 

impl<Conf: Config> BitSet<Conf> {
    /// Serialize container to a binary format.
    /// 
    /// # Format
    /// 
    /// In little endian.
    /// ```text
    /// lvl0_mask|[lvl1_mask;..]|[data;..]
    /// ```
    pub fn serialize(&self, w: &mut impl Write) -> std::io::Result<()> {
        // lvl0_mask
        let lvl0_mask = self.0.level0.mask(); 
        w.write_all(lvl0_mask.to_le_bytes().as_ref())?;
        
        // [lvl1_mask;..]
        let ctrl = lvl0_mask.traverse_bits(|i| -> ControlFlow<_> {
            let lvl1_block_index = unsafe{ self.0.level0.get_or_zero(i).as_usize() };
            let lvl1_block = unsafe{ self.0.level1.blocks().get_unchecked(lvl1_block_index) };
            
            let res = w.write_all(lvl1_block.mask().to_le_bytes().as_ref());
            match res {
                Ok(_) => ControlFlow::Continue(()),
                Err(e) => ControlFlow::Break(e)
            }
        });
        if let Some(e) = ctrl.break_value() {
            return Err(e);
        }
        
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
    
    // TODO: try to use &[u8] instead of Read? For performance. 
    pub fn deserialize(r: &mut impl Read) -> std::io::Result<Self> {
        // Level 0
        let level0: Level0Block<Conf> = {
            let mask = read_mask(r)?;
            make_hierarchy_block(mask, &mut Primitive::ONE)
        };
        
        // Level 1
        let (level1, data_blocks_len) = {
            let len = level0.mask().count_ones();
            let mut blocks = Vec::with_capacity(len+1);
            blocks.push(Default::default());    // Insert empty lvl block
            let mut index_offset = Primitive::ONE;  // one for empty data block
            for _ in 0..len {
                let mask = read_mask(r)?;
                let block: Level1Block<Conf> = make_hierarchy_block(mask, &mut index_offset);
                blocks.push(block); // TODO: faster
            }
            (
                unsafe{ Level::from_blocks_unchecked(blocks) }, 
                index_offset.as_usize()-1
            )
        };
        
        // Data level
        let data = {
            let mut blocks = Vec::with_capacity(data_blocks_len+1);
            blocks.push(Default::default());    // insert empty DataBlock
            for _ in 0..data_blocks_len{
                let mask = read_mask(r)?;
                let block: LevelDataBlock<Conf> = unsafe{Block::from_parts(
                    mask, []
                )};
                blocks.push(block);
            }
            unsafe{ Level::from_blocks_unchecked(blocks) }
        };
        
        Ok(Self(RawBitSet{
            level0, level1, data,
            phantom: Default::default(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use itertools::assert_equal;
    use super::*;
    
    #[test]
    fn simple_serialize_test(){
        use crate::config;
        
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