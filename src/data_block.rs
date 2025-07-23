use std::ops::ControlFlow;
use crate::bit_queue::BitQueue;
use crate::BitBlock;
use crate::config::Config;

#[inline]
pub fn data_block_start_index<Conf: Config>(level0_index: usize, level1_index: usize) -> usize{
    let level0_offset = level0_index << (Conf::DataBitBlock::SIZE_POT_EXPONENT + Conf::Level1BitBlock::SIZE_POT_EXPONENT);
    let level1_offset = level1_index << (Conf::DataBitBlock::SIZE_POT_EXPONENT);
    level0_offset + level1_offset
}

/// Traversable bit block with offset. 
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DataBlock<Block>{
    pub(crate) start_index: usize,
    pub(crate) bit_block: Block
}

impl<Block: BitBlock> DataBlock<Block>{
    /// # Panics
    /// 
    /// Panics if `start_index` does not match `Block` align.
    #[inline]
    pub fn new(start_index: usize, bit_block: Block) -> DataBlock<Block> {
        assert!(start_index % Block::size() == 0, "start_index does not match Block align!");
        DataBlock{start_index, bit_block}
    }
    
    /// # Safety
    /// 
    /// `start_index` must match `Block` align.
    #[inline]
    pub unsafe fn new_unchecked(start_index: usize, bit_block: Block) -> DataBlock<Block> {
        DataBlock{start_index, bit_block}
    }
    
    /// Destruct `DataBlock` into `(start_index, bit_block)`.
    #[inline]
    pub fn into_parts(self) -> (usize, Block) {
        (self.start_index, self.bit_block)
    } 
    
    // TODO: remove
    /// traverse approx. 15% faster then iterator
    #[inline]
    pub fn traverse<F, B>(&self, mut f: F) -> ControlFlow<B>
    where
        F: FnMut(usize) -> ControlFlow<B>
    {
        self.bit_block.traverse_bits(|index| f(self.start_index + index))
    }
    
    #[inline]
    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(usize)
    {
        let _ = self.traverse(move |index| -> ControlFlow<()> {
            f(index);
            ControlFlow::Continue(())
        });
    }

    #[inline]
    pub fn iter(&self) -> DataBlockIter<Block>{
        DataBlockIter{
            start_index: self.start_index,
            bit_block_iter: self.bit_block.clone().into_bits_iter()
        }
    }
    
    /// Calculate elements count in DataBlock.
    /// 
    /// On most platforms, this should be faster then manually traversing DataBlock
    /// and counting elements. It use hardware accelerated "popcnt",
    /// whenever possible. 
    #[inline]
    pub fn len(&self) -> usize {
        self.bit_block.count_ones()
    }
    
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bit_block.is_zero()
    }
}

impl<Block: BitBlock> IntoIterator for DataBlock<Block>{
    type Item = usize;
    type IntoIter = DataBlockIter<Block>;

    /// This is actually no-op fast.
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        DataBlockIter{
            start_index: self.start_index,
            bit_block_iter: self.bit_block.into_bits_iter()
        }
    }
}

/// [DataBlock] iterator.
#[derive(Clone)]
pub struct DataBlockIter<Block: BitBlock>{
    pub(crate) start_index: usize,
    pub(crate) bit_block_iter: Block::BitsIter
}

impl<Block: BitBlock> DataBlockIter<Block>{
    /// Stable version of [try_for_each].
    /// 
    /// traverse approx. 15% faster then iterator
    /// 
    /// [try_for_each]: std::iter::Iterator::try_for_each
    #[inline]
    pub fn traverse<F, B>(self, mut f: F) -> ControlFlow<B>
    where
        F: FnMut(usize) -> ControlFlow<B>
    {
        self.bit_block_iter.traverse(|index| f(self.start_index + index))
    }    
}

impl<Block: BitBlock> Iterator for DataBlockIter<Block>{
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.bit_block_iter.next().map(|index|self.start_index + index)
    }

    #[inline]
    fn for_each<F>(self, mut f: F)
    where
        F: FnMut(Self::Item)
    {
        let _ = self.traverse(|index| -> ControlFlow<()> {
            f(index);
            ControlFlow::Continue(())
        });
    }
}

#[cfg(test)]
mod test{
    use super::*;

    #[cfg(feature = "serde")]
    #[test]
    pub fn test_serde(){
        let bitblock = 14965686686284719936u64;
        let block = DataBlock::new(512, bitblock);
        
        // Serialize the person to a JSON string
        let json = serde_json::to_string(&block).unwrap();
        println!("Serialized JSON: {}", json);
    
        // Deserialize the JSON string back to a Person struct
        let deserialized_block: DataBlock<_> = serde_json::from_str(&json).unwrap();
        println!("Deserialized struct: {:?}", deserialized_block);
        
        assert_eq!(block, deserialized_block);
    }
    
    #[cfg(all(feature = "serde", feature = "simd"))]
    #[test]
    pub fn test_serde_simd(){
        use wide::u64x4;
        let bitblock = u64x4::new([16559505904331192437, 5239095950924718615, 11898057780972316399, 5585922389790652141]);
        let block = DataBlock::new(512, bitblock);
        
        // Serialize the person to a JSON string
        let json = serde_json::to_string(&block).unwrap();
        println!("Serialized JSON: {}", json);
    
        // Deserialize the JSON string back to a Person struct
        let deserialized_block: DataBlock<_> = serde_json::from_str(&json).unwrap();
        println!("Deserialized struct: {:?}", deserialized_block);
        
        assert_eq!(block, deserialized_block);
    }
    
}