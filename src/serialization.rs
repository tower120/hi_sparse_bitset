//! All read and writes in LE.

use std::{cmp, io::{Read, Write}, marker::PhantomData, primitive, slice};

use crate::{
    BitBlock,
    config::*,
    primitive::Primitive
};

pub const SERIALIZATION_FORMAT_VER: u16 = 2;

const MAX_PADDING: usize = 64;

pub(crate) struct Offsets<Conf>{
    lvl1_bitcounts_offset: usize,
    data_offset: usize,
    phantom: PhantomData<Conf>
}
impl<Conf: Config> Offsets<Conf> {
    const LVL0_MASKS_OFFSET: usize = {
        let mut offset = 8;
        offset += get_padding_for::<Lvl0Mask<Conf>>(offset);
        offset
    };

    const LVL0_BITCOUNTS_OFFSET: usize = {
        let mut offset = Self::LVL0_MASKS_OFFSET;
        offset += get_padding_for::<Lvl0Index<Conf>>(offset);
        offset
    };

    const LVL1_MASKS_OFFSET: usize = {
        let mut offset = Self::LVL0_BITCOUNTS_OFFSET;
        offset += get_padding_for::<Lvl1Mask<Conf>>(offset);
        offset
    };

    #[inline]
    pub fn new(lvl1_len: usize) -> Self {
        let lvl1_bitcounts_offset = {
            let mut offset = Self::LVL1_MASKS_OFFSET;
            offset += lvl1_len * size_of::<Lvl1Mask<Conf>>();
            offset += get_padding_for::<Lvl1Index<Conf>>(offset);
            offset
        };
        let data_offset = {
            let mut offset = lvl1_bitcounts_offset;
            offset += lvl1_len * size_of::<Lvl1Index<Conf>>();
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
    pos % align_of::<T>()
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
    pub fn read_version(&mut self) -> std::io::Result<u16> {
        let version: u16 = self.read_primitive()?;
        if version != SERIALIZATION_FORMAT_VER{
            use std::io::*;
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "Data version format mismatch. Expected {:}, read {:}.",
                    SERIALIZATION_FORMAT_VER, version
                ),
            ));
        }
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