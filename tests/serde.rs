use rand::{thread_rng, Rng};

/// For issue https://github.com/tower120/hi_sparse_bitset/pull/47
#[test]
fn regression_deserialization_256bit_arithmetic_overflow() {
    type BitSet = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_256bit>;
    
    let mut rng = thread_rng();
    let bitset: BitSet = (0..BitSet::max_capacity())
        .filter(|_| rng.gen())
        .collect();
    let mut buffer = Vec::new();
    bitset.serialize(&mut buffer).unwrap();
    assert_eq!(bitset, BitSet::deserialize(&mut buffer.as_slice()).unwrap());
}
