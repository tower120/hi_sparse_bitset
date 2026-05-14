//! All read and writes in LE.

use std::{
    fmt, slice,
    io::{Read, Write},
    marker::PhantomData,
    mem::{self, MaybeUninit},
};

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

impl fmt::Display for AccessError{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
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
    pub fn write_mask<T: BitBlock>(&mut self, mask: &T) -> std::io::Result<()> {
        self.write_masks(std::array::from_ref(mask))
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

pub(crate) struct BufferedWriter<'a, W: Write, T: Copy>{
    writer: &'a mut Writer<W>,
    buf: [MaybeUninit<T>; 64],
    buf_len: usize,
}

impl<'a, W: Write, T: Copy> BufferedWriter<'a, W, T>{
    #[inline]
    pub fn new(writer: &'a mut Writer<W>) -> Self{
        Self{
            writer,
            buf: unsafe{MaybeUninit::uninit().assume_init()},
            buf_len: 0,
        }
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        let slice = unsafe{
            slice::from_raw_parts(
                self.buf.as_ptr().cast::<u8>(),
                self.buf.len() * size_of::<T>()
            )
        };
        self.writer.write_buf(slice)?;
        self.buf_len = 0;
        Ok(())
    }

    #[inline]
    fn write_impl(&mut self, item: T) -> std::io::Result<()> {
        if self.buf_len == self.buf.len(){
            self.flush()?;
        }
        let element = unsafe{
            self.buf.get_unchecked_mut(self.buf_len)
        };
        element.write(item);
        self.buf_len += 1;
        Ok(())
    }

    #[inline]
    pub fn write_primitive(&mut self, item: T) -> std::io::Result<()>
    where
        T: Primitive
    {
        self.write_impl(item.to_le())
    }

    #[inline]
    pub fn write_mask(&mut self, item: T) -> std::io::Result<()>
    where
        T: BitBlock
    {
        self.write_impl(item.to_le())
    }

    #[inline]
    pub fn close(mut self) -> std::io::Result<()> {
        self.flush()
    }
}

impl<'a, W: Write, T: Copy> Drop for BufferedWriter<'a, W, T>{
    #[inline]
    fn drop(&mut self) {
        assert_eq!(self.buf_len, 0, "close() not called before destruction!");
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
    fn foreach_read_item<T, F>(&mut self, len: usize, mut f: F)
        -> std::io::Result<()>
    where
        F: FnMut(&T)
    {
        const BUFFER_SIZE: usize = 64;
        let mut buf: [MaybeUninit<T>; BUFFER_SIZE] = unsafe{MaybeUninit::uninit().assume_init()};

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

        chunked(len, BUFFER_SIZE, |size: usize| -> std::io::Result<()> {
            let slice: &mut [T] = unsafe{ slice::from_raw_parts_mut(
                buf.as_mut_ptr().cast(),
                size
            ) };

            self.read_buf_le(slice)?;

            // mem::transmute for array_assume_init
            let slice: &[T] = unsafe{mem::transmute(buf.get_unchecked(..size))};
            for item in slice{
                f(item);
            }
            Ok(())
        })
    }

    #[inline]
    pub fn foreach_read_mask<T, F>(&mut self, len: usize, mut f: F)
        -> std::io::Result<()>
    where
        F: FnMut(&T)
    {
        self.foreach_read_item(len, |item: &T|{
            #[cfg(target_endian = "little")]
            f(item);

            #[cfg(target_endian = "big")]
            unimplemented!();
        })
    }

    #[inline]
    pub fn skip_n<T>(&mut self, n: usize) -> std::io::Result<()> {
        self.foreach_read_item(n, |_: &T|{})
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
