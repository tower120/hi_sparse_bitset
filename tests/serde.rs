use rand::{thread_rng, Rng};

type BitSet = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_256bit>;

#[test]
fn serde() {
    let mut rng = thread_rng();
    let bitset: BitSet = (0..BitSet::max_capacity())
        .filter(|_| rng.gen())
        .collect();
    let mut buffer = Vec::new();
    bitset.serialize(&mut buffer).unwrap();
    assert_eq!(bitset, BitSet::deserialize(&mut buffer.as_slice()).unwrap());
}
