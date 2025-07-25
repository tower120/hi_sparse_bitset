use std::io::Cursor;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::{Rng, SeedableRng};
use hi_sparse_bitset::{BitSet, config};

type HiBitSet = BitSet<config::_128bit>;

pub fn bench_iter(c: &mut Criterion) {
    let mut rng = rand::rngs::StdRng::seed_from_u64(0xe15bb9db3dee3a0f);
    
    // fill
    let mut bitset = HiBitSet::default();
    for _ in 0..1_000_000 {
        let i = rng.gen_range(0..HiBitSet::max_capacity());
        bitset.insert(i);
    }
    let mut serialized = Cursor::new(Vec::new());
    bitset.serialize(&mut serialized).unwrap();
    let serialized = serialized.into_inner();
    
    c.bench_function("deserialize", |b| b.iter(|| HiBitSet::deserialize(black_box(&mut Cursor::new(&serialized) ))));
}

criterion_group!(benches_iter, bench_iter);
criterion_main!(benches_iter);