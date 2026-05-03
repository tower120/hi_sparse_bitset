/// For issue https://github.com/tower120/hi_sparse_bitset/pull/47
#[test]
#[cfg_attr(miri, ignore)]
fn regression_deserialization_256bit_arithmetic_overflow() {
    type BitSet = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_256bit>;

    use rand::prelude::*;
    let mut rng = rand::rng();
    let bitset: BitSet = (0..BitSet::max_capacity())
        .filter(|_| rng.random())
        .collect();
    let mut buffer = Vec::new();
    bitset.serialize(&mut buffer).unwrap();
    assert_eq!(bitset, BitSet::deserialize(&mut buffer.as_slice()).unwrap());
}
