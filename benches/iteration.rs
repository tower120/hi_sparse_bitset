use criterion::{black_box, Criterion, criterion_group, criterion_main};

type HiSparseBitset = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_64bit>;

fn iteration(set: &HiSparseBitset) -> u64{
    let mut s = 0;
    for data in set.block_iter(){
        s += data.len() as u64;
    }
    s
}

pub fn bench_iter(c: &mut Criterion) {
    let mut set: HiSparseBitset = Default::default();
    for i in 0..3000{
        set.insert(i*64);
    }
    
    c.bench_function("hi_sparse_bitset iter", |b| b.iter(|| iteration(black_box(&set))));
}

criterion_group!(benches_iter, bench_iter);
criterion_main!(benches_iter);