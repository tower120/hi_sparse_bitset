mod common;

use criterion::{black_box, Criterion, criterion_group, criterion_main};
use crate::common::bench;

type HiSparseBitset = hi_sparse_bitset::SmallBitSet<hi_sparse_bitset::config::_128bit>;

fn hi_sparse_bitset_insert(in_block: usize) -> HiSparseBitset{
    let mut set: HiSparseBitset = Default::default();
    for lvl0 in 0..128 {
        for lvl1 in 0..6 {
            let offset = lvl0*128*128 + lvl1*128;
            for i in 0..in_block{
                set.insert(offset + i);    
            }             
        }
    }
    set    
}

pub fn bench_iter(c: &mut Criterion) {
    c.bench_function("hi_sparse_bitset insert", |b| b.iter(|| hi_sparse_bitset_insert(black_box(80))));
}

criterion_group!(benches_iter, bench_iter);
criterion_main!(benches_iter);