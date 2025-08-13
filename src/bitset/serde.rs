use std::fmt;
use std::fmt::Formatter;
use std::io::Cursor;
use std::marker::PhantomData;
use serde::{Deserialize, Serialize};
use crate::{BitBlock, BitSet};
use crate::bitset::level::IBlock;
use crate::config::Config;

impl<Conf: Config> Serialize for BitSet<Conf>{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        use arrayvec::ArrayVec;
        
        // SBO
        const STACK_BUFFER_LEN: usize = 4096;
        let mut on_stack: ArrayVec<u8, STACK_BUFFER_LEN>;
        let mut on_heap : Vec<u8>;
        
        // real_len <= approx_len
        let approx_len = 
            Conf::DataBitBlock::size()                                               // root block
            + (1 + self.level0.mask().count_ones()) * Conf::Level1BitBlock::size()   // lvl1 blocks
            + (1 + self.data.blocks().len())        * Conf::DataBitBlock::size();    // approx data blocks
        
        // There should be no errors at all.
        let array = if approx_len <= STACK_BUFFER_LEN {
            on_stack = ArrayVec::new();
            unsafe{ self.serialize(&mut on_stack).unwrap_unchecked(); }
            on_stack.as_slice()
        } else {
            on_heap = Vec::with_capacity(approx_len);
            unsafe{ self.serialize(&mut on_heap).unwrap_unchecked(); }
            on_heap.as_slice()
        };
        
        if serializer.is_human_readable() {
            // collect_str instead of serialize_str allow to omit constructing
            // intermediate base64 encoded String.
            use base64::{display::Base64Display, engine::general_purpose::STANDARD};
            serializer.collect_str(&Base64Display::new(array, &STANDARD))
        } else {
            // we assume there is an efficient byte encoder in serializer.
            serializer.serialize_bytes(array)
        }
    }
}

impl<'de, Conf: Config> Deserialize<'de> for BitSet<Conf>{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>
    {
        use serde::de::Error;
        
        if deserializer.is_human_readable() {
            struct Visitor<Conf>(PhantomData<Conf>);
            impl<'de, Conf: Config> serde::de::Visitor<'de> for Visitor<Conf> {
                type Value = BitSet<Conf>;

                fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
                    formatter.write_str("a string")
                }

                fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
                    use base64::{read::DecoderReader, engine::general_purpose::STANDARD};
                    let mut decoder = DecoderReader::new(Cursor::new(v), &STANDARD);
                    BitSet::deserialize(&mut decoder).map_err(Error::custom)
                }
            }
            deserializer.deserialize_str(Visitor(PhantomData))
        } else {
            struct Visitor<Conf>(PhantomData<Conf>);
            impl<'de, Conf: Config> serde::de::Visitor<'de> for Visitor<Conf> {
                type Value = BitSet<Conf>;
            
                fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
                    formatter.write_str("a byte slice")
                }

                fn visit_bytes<E: Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                    BitSet::deserialize(&mut Cursor::new(v)).map_err(Error::custom)
                }
            }            
            deserializer.deserialize_bytes(Visitor(PhantomData))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Seek;
    use itertools::assert_equal;
    use crate::config;
    use super::*;
    
    #[test]
    fn simple_serde_json_test(){
        let mut bitset: BitSet<config::_64bit> = Default::default();
        bitset.insert(100);
        bitset.insert(5720);
        bitset.insert(219347);

        let serialized = serde_json::to_string(&bitset).unwrap();
        println!("Serialized {:?}", serialized);
        
        let deserialized_bitset: BitSet<config::_64bit> = serde_json::from_str(&serialized).unwrap();
        println!("Deserialized {:?}", deserialized_bitset);
        
        assert_eq!(bitset, deserialized_bitset);
        assert_equal(bitset.iter(), deserialized_bitset.iter());    // check by iter too.
    }
    
    #[test]
    fn simple_serde_bincode_test(){
        let mut bitset: BitSet<config::_64bit> = Default::default();
        bitset.insert(100);
        bitset.insert(5720);
        bitset.insert(219347);

        let config = bincode::config::standard();
        let serialized = bincode::serde::encode_to_vec(&bitset, config).unwrap();
        println!("Serialized {:?}", serialized);
        
        let deserialized_bitset: BitSet<config::_64bit> = bincode::serde::decode_from_slice(&serialized, config).unwrap().0;
        println!("Deserialized {:?}", deserialized_bitset);
        
        assert_eq!(bitset, deserialized_bitset);
        assert_equal(bitset.iter(), deserialized_bitset.iter());    // check by iter too.
    }
    
    #[test]
    fn serde_json_w_file() -> anyhow::Result<()> {
        type BitSet = crate::BitSet<config::_64bit>;
        let set = BitSet::from_iter(0..260_000usize);
        
        let mut file = tempfile::tempfile()?;
        serde_json::to_writer(&mut file, &set)?;
        
        file.rewind()?;
        let _s: BitSet = serde_json::from_reader(file)?;
        Ok(())
    }    
}