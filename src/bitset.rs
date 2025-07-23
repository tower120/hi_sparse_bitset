mod serialization;
mod block;
mod level;
mod raw;
mod derive_raw;

use crate::config::Config;
use block::Block;
use derive_raw::derive_raw;
use crate::BitSetBase;

type Level0Block<Conf> = Block<
    <Conf as Config>::Level0BitBlock, 
    <Conf as Config>::Level0BlockIndices
>;
type Level1Block<Conf> = Block<
    <Conf as Config>::Level1BitBlock,
    <Conf as Config>::Level1BlockIndices
>;
type LevelDataBlock<Conf> = Block<
    <Conf as Config>::DataBitBlock, [usize;0]
>;

type RawBitSet<Conf> = raw::RawBitSet<
    Conf,
    Level0Block<Conf>,
    Level1Block<Conf>,
    LevelDataBlock<Conf>
>;

/// Hierarchical sparse bitset.
///
/// Tri-level hierarchy. Highest uint it can hold
/// is [Level0BitBlock]::size() * [Level1BitBlock]::size() * [DataBitBlock]::size().
///
/// Only last level contains blocks of actual data. Empty(skipped) data blocks
/// are not allocated.
///
/// Structure optimized for intersection speed. 
/// _(Other inter-bitset operations are in fact fast too - but intersection has lowest algorithmic complexity.)_
/// Insert/remove/contains is fast O(1) too.
/// 
/// [Level0BitBlock]: crate::config::Config::Level0BitBlock
/// [Level1BitBlock]: crate::config::Config::Level1BitBlock
/// [DataBitBlock]: crate::config::Config::DataBitBlock
pub struct BitSet<Conf: Config>(
    RawBitSet<Conf>
);
impl<Conf: Config> BitSetBase for BitSet<Conf> {
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = true;
}
derive_raw!(
    impl<Conf> BitSet<Conf> as RawBitSet<Conf> where Conf: Config  
);

/*#[cfg(feature = "serde")]
impl<'de, Conf> Deserialize<'de> for BitSet<Conf>
where
    Conf: Config,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>
    {
        
        struct Visitor<Conf>(PhantomData<Conf>);
        impl<'de, Conf: Config> serde::de::Visitor<'de> for Visitor<Conf> {
            type Value = ();
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a tuple of (u8, String, Vec<u8>)")
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let lvl0: Conf::Level0BitBlock = seq.next_element()?.ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                
                seq.
                
                for _ in 0..lvl0.count_ones() {
                    if let Some(bitblock) = seq.next_element::<Conf::Level1BitBlock>()? {
                        println!("lvl1: {:?}", bitblock);
                        // ..
                    }
                }                
                
                //let lvl1: Vec<Conf::Level1BitBlock> = seq.next_element()?.ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                let data: Vec<Conf::DataBitBlock> = seq.next_element()?.ok_or_else(|| serde::de::Error::invalid_length(2, &self))?;
                
                println!("lvl0: {:?}", lvl0);
                
                Ok(())
            }
        }
        deserializer.deserialize_seq(Visitor::<Conf>(PhantomData));
        
        #[repr(transparent)]
        struct BlockWrapper<Mask, BlockIndices>(Block<Mask, BlockIndices>);
        impl<'de, Mask, BlockIndices> Deserialize<'de> for BlockWrapper<Mask, BlockIndices>
        where
            Mask: BitBlock,
            BlockIndices: PrimitiveArray
        {
            #[inline]
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>
            {
                let mask = Mask::deserialize(deserializer)?;
                
                let mut block_indices: BlockIndices = unsafe{MaybeUninit::zeroed().assume_init()};
                
                // Fill bloc indices skipping 0.
                // We know that blocks 
                {
                    let len = mask.count_ones();
                    for i in 0..len {
                        // block_indices.as_mut()[i] = (i+1).into();
                    }
                }
                
                
                let block = unsafe { Block::from_parts(mask, block_indices) };
                
                Ok(BlockWrapper(block))
            }
        }

        /*let (lvl0, lvl1, data): (Conf::Level0BitBlock, Vec<BlockWrapper<Conf::Level1BitBlock, Conf::Level1BlockIndices>>, Vec<Conf::DataBitBlock>) = Deserialize::deserialize(deserializer)?;
        
        println!("lvl0: {:?}", lvl0);
        //println!("lvl1: {:?}", lvl1);
        println!("data: {:?}", data);*/
        Ok(Self::default())
    }
}*/