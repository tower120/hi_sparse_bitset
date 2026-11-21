use core::{fmt, marker::PhantomData};
use crate::config::*;

/// Current serialization format version.
pub const SERIALIZATION_FORMAT_VER: u16 = 3;

const MAX_PADDING: usize = 64;

#[derive(Debug)]
pub enum AccessError{
    /// (requested align)
    Unaligned(usize),

    /// (version found)
    FormatMismatch(u16),

    /// Unexpected end of data.
    ///
    /// An example of this - serialized byte slice that is shorter than its declared data.
    UnexpectedEof,

    // TODO: This should be core::io::Error - but it is unstable now.
    #[cfg(feature = "std")]
    IOError(std::io::Error)
}

#[cfg(feature = "std")]
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
    pub const HEADER_SIZE: usize = 8;

    pub const LVL0_MASK_OFFSET: usize = {
        let mut offset = Self::HEADER_SIZE;
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

#[cfg(feature = "std")]
mod io {
use super::*;
use crate::{BitBlock, primitive::Primitive};
use std::{slice, io::{Read, Write}, mem::{self, MaybeUninit}};

#[inline]
fn chunked<E, F, const CHUNK_SIZE: usize>(len: usize, mut f: F)
    -> F::Output
where
    F: FnMut(usize/*chunk idx*/, usize/*size*/) -> Result<(), E>
{
    const {
        // We want log2 only sizes (for fast div).
        // This also checks for non-zero.
        assert!(CHUNK_SIZE.is_power_of_two());
    }

    let full_chunks = len / CHUNK_SIZE;
    for i in 0..full_chunks {
        f(i, CHUNK_SIZE)?;
    }

    let rem = len % CHUNK_SIZE;
    if rem != 0{
        f(full_chunks, rem)?;
    }
    Ok(())
}

pub(crate) struct Writer<W>{
    write: W,
    pos: usize
}

#[allow(dead_code)]
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
    pub fn write_buf(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.write.write_all(buf)?;
        self.pos += buf.len();
        Ok(())
    }

    /// # Safety
    ///
    /// `items` MUST be "truly" `ExactSizeIterator`.
    #[inline]
    pub unsafe fn write_buffered<T, const BUFFER_SIZE: usize>(
        &mut self,
        mut items: impl Iterator<Item = T> + ExactSizeIterator
    ) -> std::io::Result<()> {
        const{ assert!(!mem::needs_drop::<T>()); }
        let mut chunk: [MaybeUninit<T>; BUFFER_SIZE] = unsafe{
            MaybeUninit::uninit().assume_init()
        };

        chunked::<_, _, BUFFER_SIZE> (
            items.len(),
            |_, chunk_size: usize| -> std::io::Result<()> {
                for i in 0..chunk_size {
                    unsafe{
                        let item = items.next().unwrap_unchecked();
                        chunk.get_unchecked_mut(i).write(item);
                    }
                }

                let bytes = unsafe{std::slice::from_raw_parts(
                    chunk.as_ptr().cast::<u8>(),
                    chunk_size * size_of::<T>()
                )};
                self.write_buf(bytes)?;
                Ok(())
            }
        )
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
        unsafe {
            self.write_buffered::<T, 512>(
                buf.iter().map(|primitive| primitive.to_le())
            )
        }
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
        unsafe {
            self.write_buffered::<T, 64>(
                buf.iter().map(|mask| mask.to_le())
            )
        }
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
                self.buf_len * size_of::<T>()
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

#[allow(dead_code)]
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

    /// Read to LE `buf` in chunks of `BUFFER_SIZE` size.
    /// After each chunk read, calls `f` with read portion of `buf` as param.
    #[inline]
    fn read_buf_le_chunked<T, F, const BUFFER_SIZE: usize>(
        &mut self, buf: &mut [T], mut f: F
    ) -> std::io::Result<()>
    where
        F: FnMut(&mut [T])
    {
        chunked::<_, _, BUFFER_SIZE>(
            buf.len(),
            |chunk_idx: usize, chunk_size: usize| -> std::io::Result<()> {
                let slice: &mut [T] = unsafe{ slice::from_raw_parts_mut(
                    buf.as_mut_ptr().add(chunk_idx * BUFFER_SIZE),
                    chunk_size
                ) };

                self.read_buf_le(slice)?;
                f(slice);
                Ok(())
            }
        )
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
        {
            self.read_buf_le_chunked::<_, _, 512>(primitives, |primitives_chunk| {
                for primitive in primitives_chunk.iter_mut() {
                    *primitive = primitive.swap_bytes();
                }
            })
        }
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

        chunked::<_, _, BUFFER_SIZE>(len, |_, size: usize| -> std::io::Result<()> {
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
        T: BitBlock,
        F: FnMut(&T)
    {
        self.foreach_read_item(len, |item: &T|{
            #[cfg(target_endian = "little")]
            f(item);

            #[cfg(target_endian = "big")]
            f(&T::from_le(*item));
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
        {
            self.read_buf_le_chunked::<_, _, 64>(masks, |masks_chunk|{
                for mask in masks_chunk.iter_mut() {
                    *mask = T::from_le(*mask);
                }
            })
        }
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
    use super::*;

    #[test]
    fn chunked_test(){
        /// Returns (chunk idx, size) of each call that `chunked()` makes
        /// for `len` items, with chunks of 4 items.
        fn chunks(len: usize) -> Vec<(usize, usize)> {
            let mut chunks = Vec::new();
            chunked::<(), _, 4>(len, |chunk_idx, size| {
                chunks.push((chunk_idx, size));
                Ok(())
            }).unwrap();
            chunks
        }

        // No items - no calls.
        assert!(chunks(0).is_empty());
        // Less than one chunk - leftover only.
        assert_eq!(chunks(3), [(0, 3)]);
        // Full chunks, no leftover.
        assert_eq!(chunks(4), [(0, 4)]);
        assert_eq!(chunks(8), [(0, 4), (1, 4)]);
        // Full chunks, then leftover.
        assert_eq!(chunks(5), [(0, 4), (1, 1)]);
        assert_eq!(chunks(11), [(0, 4), (1, 4), (2, 3)]);
    }

    #[cfg(feature = "simd")]
    #[test]
    fn offsets_test(){
        type Conf = crate::config::_256bit;
        type OF = Offsets::<Conf>;

        println!("{:}", OF::LVL0_MASK_OFFSET);
        println!("{:}", OF::LVL0_BITCOUNTS_OFFSET);
        println!("{:}", OF::LVL1_MASKS_OFFSET);

        let offsets = OF::new(16);
        println!("{:}", offsets.lvl1_bitcounts_offset);
        println!("{:}", offsets.data_offset);
    }
}
}

#[cfg(feature = "std")]
pub(crate) use io::*;
