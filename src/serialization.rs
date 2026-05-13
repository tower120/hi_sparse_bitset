//! All read and writes in LE.

use std::{cmp, io::{Read, Write}, marker::PhantomData, primitive, slice};

use crate::{
    BitBlock,
    config::*,
    primitive::Primitive
};

/// Current serialization format version.
pub const SERIALIZATION_FORMAT_VER: u16 = 3;

const MAX_PADDING: usize = 64;

#[derive(Debug)]
pub enum AccessError{
    /// (requested align)
    Unaligned(usize),

    /// (version found)
    FormatMismatch(u16),

    IOError(std::io::Error)
}

impl From<std::io::Error> for AccessError{
    #[inline]
    fn from(value: std::io::Error) -> Self {
        Self::IOError(value)
    }
}

#[derive(Clone)]
pub(crate) struct Offsets<Conf>{
    pub lvl1_bitcounts_offset: usize,
    pub data_offset: usize,
    phantom: PhantomData<Conf>
}
impl<Conf: Config> Offsets<Conf> {
    pub const LVL0_MASK_OFFSET: usize = {
        let mut offset = 8;
        offset += get_padding_for::<Lvl0Mask<Conf>>(offset);
        offset
    };

    pub const LVL0_BITCOUNTS_OFFSET: usize = {
        let mut offset = Self::LVL0_MASK_OFFSET;
        offset += size_of::<Lvl0Mask<Conf>>();
        offset += get_padding_for::<Lvl0Index<Conf>>(offset);
        offset
    };

    pub const LVL1_MASKS_OFFSET: usize = {
        let mut offset = Self::LVL0_BITCOUNTS_OFFSET;
        offset += size_of::<Lvl0Index<Conf>>() * 8;
        offset += get_padding_for::<Lvl1Mask<Conf>>(offset);
        offset
    };

    #[inline]
    pub const fn len(&self, data_len: usize) -> usize{
        self.data_offset + data_len * size_of::<DataMask<Conf>>()
    }

    #[inline]
    pub const fn new(lvl1_len: usize) -> Self {
        let lvl1_bitcounts_offset = {
            let mut offset = Self::LVL1_MASKS_OFFSET;
            offset += lvl1_len * size_of::<Lvl1Mask<Conf>>();
            offset += get_padding_for::<Lvl1Index<Conf>>(offset);
            offset
        };
        let data_offset = {
            let mut offset = lvl1_bitcounts_offset;
            offset += lvl1_len * (size_of::<Lvl1Mask<Conf>>() / 8) * size_of::<Lvl1Index<Conf>>();
            offset += get_padding_for::<DataMask<Conf>>(offset);
            offset
        };
        Self{
            lvl1_bitcounts_offset,
            data_offset,
            phantom: PhantomData
        }
    }
}

#[inline]
const fn get_padding_for<T>(pos: usize) -> usize {
    const{
        assert!(align_of::<T>() <= MAX_PADDING);
        assert!(align_of::<T>().is_power_of_two());
    }
    // From https://en.wikipedia.org/wiki/Data_structure_alignment#Computing_padding
    let padding = -(pos as isize) & (align_of::<T>() as isize - 1);
    padding as usize
}

#[inline]
pub(crate) fn check_version(version: u16) ->  Result<(), AccessError>{
    if version != SERIALIZATION_FORMAT_VER{
        return Err(AccessError::FormatMismatch(version));
    }
    Ok(())
}

pub(crate) struct Writer<W>{
    write: W,
    pos: usize
}

impl<W: Write> Writer<W>{
    #[inline]
    pub fn new(write: W) -> Self{
        Self{write, pos: 0}
    }

    #[inline]
    pub fn pos(&self) -> usize{
        self.pos
    }

    #[inline]
    pub fn write_buf(&mut self, buf: &[u8]) -> std::io::Result<()>{
        self.write.write_all(buf)?;
        self.pos += buf.len();
        Ok(())
    }

    #[inline]
    pub fn write_primitive<T: Primitive>(&mut self, primitive: T) -> std::io::Result<()>{
        self.write_buf(primitive.to_le_bytes().as_ref())
    }

    #[inline]
    pub fn write_primitives<T: Primitive>(&mut self, buf: &[T]) -> std::io::Result<()> {
        #[cfg(target_endian = "little")]
        {
            let bytes = unsafe{std::slice::from_raw_parts(
                buf.as_ptr().cast::<u8>(),
                buf.len() * size_of::<T>()
            )};
            self.write_buf(bytes)
        }

        #[cfg(target_endian = "big")]
        unimplemented!("TODO: convert to chunks and write_all that chunks");
    }

    #[inline]
    pub fn write_masks<T: BitBlock>(&mut self, buf: &[T]) -> std::io::Result<()> {
        #[cfg(target_endian = "little")]
        {
            let bytes = unsafe{std::slice::from_raw_parts(
                buf.as_ptr().cast::<u8>(),
                buf.len() * size_of::<T>()
            )};
            self.write_buf(bytes)
        }

        #[cfg(target_endian = "big")]
        unimplemented!("TODO: convert to chunks and write_all that chunks");
    }

    #[inline]
    pub fn write_padding_for<T>(&mut self) -> std::io::Result<()>{
        const BUFFER: [u8; MAX_PADDING] = [0; MAX_PADDING];
        let padding = get_padding_for::<T>(self.pos);
        self.write.write_all(&BUFFER[..padding])?;
        self.pos += padding;
        Ok(())
    }
}

pub(crate) struct Reader<R>{
    read: R,
    pos: usize
}

impl<R: Read> Reader<R>{
    #[inline]
    pub fn new(read: R) -> Self{
        Self{read, pos: 0}
    }

    #[inline]
    pub fn pos(&self) -> usize{
        self.pos
    }

    #[inline]
    pub fn read_primitive<T: Primitive>(&mut self) -> std::io::Result<T> {
        let mut buf = T::ZERO.to_ne_bytes();
        self.read.read_exact(buf.as_mut())?;
        self.pos += buf.as_ref().len();
        Ok(T::from_le_bytes(buf))
    }

    #[inline]
    pub fn read_version(&mut self) -> Result<u16, AccessError> {
        let version: u16 = self.read_primitive()?;
        check_version(version)?;
        Ok(version)
    }

    #[inline]
    pub fn read_mask<Mask: BitBlock>(&mut self) -> std::io::Result<Mask> {
        let mut buf = Mask::zero().to_ne_bytes();
        self.read.read_exact(buf.as_mut())?;
        self.pos += buf.as_ref().len();
        Ok(Mask::from_le_bytes(buf))
    }

    #[inline]
    pub fn read_padding_for<T>(&mut self) -> std::io::Result<()> {
        let padding = get_padding_for::<T>(self.pos);
        let mut buffer: [u8; MAX_PADDING] = [0; MAX_PADDING];
        let slice = unsafe{
            slice::from_raw_parts_mut(
                buffer.as_mut_ptr(),
                padding,
            )
        };
        self.read.read_exact(slice)?;
        self.pos += padding;
        Ok(())
    }

    #[inline]
    fn read_buf_le<T>(&mut self, buf: &mut[T])
        -> std::io::Result<()>
    {
        let len = buf.len() * size_of::<T>();
        let bytes: &mut [u8] = unsafe{
            std::slice::from_raw_parts_mut(
                buf.as_mut_ptr().cast(),
                len
            )
        };
        self.read.read_exact(bytes)?;
        self.pos += len;
        Ok(())
    }

    #[inline]
    pub fn read_primitives<T: Primitive>(&mut self, primitives: &mut[T])
        -> std::io::Result<()>
    {
        #[cfg(target_endian = "little")]
        {
            self.read_buf_le(primitives)
        }

        #[cfg(target_endian = "big")]
        unimplemented!();
    }

    #[inline]
    pub fn read_primitives_to_vec<T: Primitive>(
        &mut self, primitives: &mut Vec<T>, len: usize
    ) -> std::io::Result<()> {
        primitives.reserve_exact(len);
        let slice = unsafe{
            std::slice::from_raw_parts_mut(
                primitives.as_mut_ptr(),
                len
            )
        };
        self.read_primitives(slice)?;
        unsafe{ primitives.set_len(primitives.len() + len); }
        Ok(())
    }


    #[inline]
    pub fn read_masks<T: BitBlock>(&mut self, masks: &mut[T])
        -> std::io::Result<()>
    {
        #[cfg(target_endian = "little")]
        {
            self.read_buf_le(masks)
        }

        #[cfg(target_endian = "big")]
        unimplemented!();
    }

    #[inline]
    pub fn read_masks_to_vec<T: BitBlock>(&mut self, masks: &mut Vec<T>, len: usize)
        -> std::io::Result<()>
    {
        masks.reserve_exact(len);
        let slice = unsafe{
            std::slice::from_raw_parts_mut(
                masks.as_mut_ptr(),
                len
            )
        };
        self.read_masks(slice)?;
        unsafe{ masks.set_len(masks.len() + len); }
        Ok(())
    }
}

#[cfg(test)]
mod tests{
    use crate::config;
    use super::*;

    #[test]
    fn offsets_test(){
        type Conf = config::_256bit;
        type OF = Offsets::<Conf>;

        println!("{:}", OF::LVL0_MASK_OFFSET);
        println!("{:}", OF::LVL0_BITCOUNTS_OFFSET);
        println!("{:}", OF::LVL1_MASKS_OFFSET);

        let offsets = OF::new(16);
        println!("{:}", offsets.lvl1_bitcounts_offset);
        println!("{:}", offsets.data_offset);
    }
}