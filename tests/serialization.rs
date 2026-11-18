//! Serialization tests for all containers:
//! `BitSet`, `ImmutableBitset` and `DirectBitset`.
#![cfg(feature = "std")]

use hi_sparse_bitset::{config, config::Config, BitSet, DirectBitset, ImmutableBitset};
use itertools::assert_equal;
use rand::prelude::*;

/// Serializes `bitset` with `BitSet` and with `ImmutableBitset`, which have
/// separate serialization implementations, and checks that both give the
/// same bytes. Then reads the bytes back with `BitSet`, `ImmutableBitset`
/// and `DirectBitset`, and checks that each one has the items of `bitset`.
fn check_serialization<Conf: Config>(bitset: &BitSet<Conf>) -> Vec<u8> {
    let mut bytes = Vec::new();
    bitset.serialize(&mut bytes).unwrap();

    let mut immutable_bytes = Vec::new();
    ImmutableBitset::<Conf>::from(bitset).serialize(&mut immutable_bytes).unwrap();
    assert_eq!(bytes, immutable_bytes);

    let deserialized = BitSet::<Conf>::deserialize(&mut bytes.as_slice()).unwrap();
    assert_eq!(*bitset, deserialized);

    let immutable = ImmutableBitset::<Conf>::deserialize(&mut bytes.as_slice()).unwrap();
    assert_equal(bitset.iter(), immutable.iter());

    let direct = DirectBitset::<Conf, &[u8]>::new(bytes.as_slice(), 0).unwrap();
    assert_equal(bitset.iter(), direct.iter());

    bytes
}

/// Serialization writes and reads masks through a buffer of 64 masks, and
/// on big-endian targets level1 bitcounts through a buffer of 512 (see
/// `BufferedWriter`, `write_buffered()` and `read_buf_le_chunked()` in
/// src/serialization.rs). This bitset fills these buffers several times,
/// with a partly filled buffer at the end. It is small enough for Miri.
fn round_trip_many_blocks<Conf: Config>() {
    // 600 items give about 600 data blocks. Only the first 3/4 of the range
    // is used, so that the number of level1 blocks is not a multiple of 64:
    // up to 48, 96 and 192 level1 blocks with the 64, 128 and 256 bit
    // configs. With 256 bits each level1 block has 4 bitcounts, so only
    // that config has more than 512 of them.
    //
    // The items are random, so that the blocks differ from each other:
    // a buffer written or read at the wrong place gives a wrong result.
    let mut rng = StdRng::seed_from_u64(0);
    let range = BitSet::<Conf>::max_capacity() / 4 * 3;
    let bitset: BitSet<Conf> = (0..600).map(|_| rng.random_range(0..range)).collect();
    check_serialization(&bitset);
}

#[test]
fn round_trip_many_blocks_64bit() {
    round_trip_many_blocks::<config::_64bit>();
}

#[cfg(feature = "simd")]
#[test]
fn round_trip_many_blocks_128bit() {
    round_trip_many_blocks::<config::_128bit>();
}

#[cfg(feature = "simd")]
#[test]
fn round_trip_many_blocks_256bit() {
    round_trip_many_blocks::<config::_256bit>();
}

/// The serialized bytes are little-endian on every target, so that they can
/// be read on a target with the other byte order. A round trip alone can't
/// show that.
#[test]
fn serialized_bytes_are_little_endian() {
    // With the 64 bit config, a level1 block has 64 data blocks of 64 items.
    // Item 1:    level1 block 0, data block 0, bit 1.
    // Item 4235: level1 block 1, data block 2, bit 11 (4235 = 4096 + 2*64 + 11).
    let bitset: BitSet<config::_64bit> = [1, 4235].into_iter().collect();

    #[rustfmt::skip]
    let expected: [u8; 64] = [
        // header: format version 3, 2 level1 blocks, 2 data blocks
        3, 0,  2, 0,  2, 0, 0, 0,
        // level0 mask: level1 blocks 0 and 1
        0b11, 0, 0, 0, 0, 0, 0, 0,
        // level0 bitcounts
        0, 0, 0, 0, 0, 0, 0, 0,
        // level1 masks: data block 0, then data block 2
        0b1,   0, 0, 0, 0, 0, 0, 0,
        0b100, 0, 0, 0, 0, 0, 0, 0,
        // level1 bitcounts (u16): 0 and 1 data blocks before each level1 block,
        // then padding
        0, 0,  1, 0,  0, 0, 0, 0,
        // data masks: bit 1, then bit 11
        0b10, 0,      0, 0, 0, 0, 0, 0,
        0,    0b1000, 0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(check_serialization(&bitset), expected);
}
