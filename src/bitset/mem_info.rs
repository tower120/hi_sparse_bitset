use std::fmt::Debug;

use crate::{BitSet, bitset::{Level0Block, Level1Block, LevelDataBlock}, config::Config};

/// [BitSet] memory usage info.
pub struct MemInfo<'a, Conf: Config>{
    pub(crate) bitset: &'a BitSet<Conf>
}

impl<'a, Conf: Config> MemInfo<'a, Conf>{
    /// Size of root block in [BitSet] structure. In Bytes.
    pub const ROOT_BLOCK_SIZE: usize = size_of::<Level0Block<Conf>>();

    /// Size of hierarchy block in [BitSet] structure. In Bytes.
    pub const HI_BLOCK_SIZE: usize = size_of::<Level1Block<Conf>>();

    /// Size of data block in [BitSet] structure. In Bytes.
    pub const DATA_BLOCK_SIZE: usize = size_of::<LevelDataBlock<Conf>>();

    /// Capacity of hierarchy block storage.
    #[inline]
    pub fn hi_blocks_cap(&self) -> usize{
        self.bitset.level1.cap()
    }

    /// Amount of hierarchy blocks actually used.
    #[inline]
    pub fn hi_blocks_len(&self) -> usize{
        self.bitset.level1.len()
    }

    /// Capacity of data block storage.
    #[inline]
    pub fn data_blocks_cap(&self) -> usize {
        self.bitset.data.cap()
    }

    /// Amount of data blocks actually used.
    #[inline]
    pub fn data_blocks_len(&self) -> usize {
        self.bitset.data.len()
    }

    /// Total amount of memory used by [BitSet]. In bytes.
    #[inline]
    pub fn mem_usage(&self) -> usize{
        size_of::<BitSet<Conf>>()
        + self.hi_blocks_cap() * Self::HI_BLOCK_SIZE
        + self.data_blocks_cap() * Self::DATA_BLOCK_SIZE
    }
}

impl<'a, Conf: Config> Debug for MemInfo<'a, Conf>{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemInfo")
        .field("hi_blocks_cap", &self.hi_blocks_cap())
        .field("hi_blocks_len", &self.hi_blocks_len())
        .field("data_blocks_cap", &self.data_blocks_cap())
        .field("data_blocks_len", &self.data_blocks_len())
        .field("mem_usage", &self.mem_usage())
        .finish()
    }
}

#[cfg(test)]
mod test{
    use crate::config;
    use super::*;

    #[test]
    fn test(){
        type Conf = config::_64bit;
        let s = BitSet::<Conf>::new();
        println!("{:#?}", s.mem_info());
    }
}