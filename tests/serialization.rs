// Serialization currently requires `std` feature.
#![cfg(feature = "std")]

use hi_sparse_bitset::{config, config::Config, BitSet, DirectBitset, ImmutableBitset};
use itertools::assert_equal;
use rand::prelude::*;

/// Check serialization/deserialization of `bitset` across all containers.
///
/// Return serialized bytes.
fn check_serialization<Conf: Config>(bitset: &BitSet<Conf>) -> Vec<u8> {
    let mut bitset_serialized = Vec::new();
    bitset.serialize(&mut bitset_serialized).unwrap();

    let mut immutable_serialized = Vec::new();
    ImmutableBitset::<Conf>::from(bitset).serialize(&mut immutable_serialized).unwrap();
    assert_eq!(bitset_serialized, immutable_serialized);

    let bitset_deserialized = BitSet::<Conf>::deserialize(&mut bitset_serialized.as_slice()).unwrap();
    assert_eq!(*bitset, bitset_deserialized);

    let immutable_deserialized = ImmutableBitset::<Conf>::deserialize(&mut bitset_serialized.as_slice()).unwrap();
    assert_equal(bitset.iter(), immutable_deserialized.iter());

    let direct = DirectBitset::<Conf, &[u8]>::new(bitset_serialized.as_slice(), 0).unwrap();
    assert_equal(bitset.iter(), direct.iter());

    bitset_serialized
}

/// Designed for miri run.
/// Bitset len is longer then in "general" `fuzzy_test()` miri run.
fn serialization_fuzzy_test<Conf: Config>() {
    let mut rng = rand::rng();
    for _ in 0..4 {
        let max = BitSet::<Conf>::max_capacity();
        let range = rng.random_range(max/4..max);
        let bitset: BitSet<Conf> = (0..600).map(|_| rng.random_range(0..range)).collect();
        check_serialization(&bitset);
    }
}

#[test]
fn serialization_fuzzy_test_64bit() {
    serialization_fuzzy_test::<config::_64bit>();
}

#[cfg(feature = "simd")]
#[test]
fn serialization_fuzzy_test_128bit() {
    serialization_fuzzy_test::<config::_128bit>();
}

#[cfg(feature = "simd")]
#[test]
fn serialization_fuzzy_test_256bit() {
    serialization_fuzzy_test::<config::_256bit>();
}

/// The serialized bytes are little-endian on every target, so that they can
/// be read on a target with the other byte order. Fuzzy testing may omit that.
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
