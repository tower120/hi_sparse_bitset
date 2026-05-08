use std::sync::Arc;

use criterion::{black_box, Criterion, criterion_group, criterion_main};
use memmap2::Mmap;

type Config = hi_sparse_bitset::config::_64bit;
type HiSparseBitset = hi_sparse_bitset::BitSet<Config>;
type MMapBitset = hi_sparse_bitset::mmap_bitset::ImmutableBitset<Config>;

fn iteration(set: &HiSparseBitset) -> u64{
    let mut s = 0;
    for data in set.block_iter(){
        s += data.len() as u64;
    }
    s
}

fn mmap_iteration(set: &MMapBitset) -> u64{
    let mut s = 0;
    for data in set.block_iter(){
        s += data.len() as u64;
    }
    s
}

pub fn bench_iter(c: &mut Criterion) {
    let mut set: HiSparseBitset = Default::default();
    for i in 0..30000{
        set.insert(i*4);
    }

    let mut file = tempfile::tempfile().unwrap();
    set.serialize(&mut file).unwrap();

    let mmap = unsafe { Mmap::map(&file).unwrap()  };
    let mmap_set = MMapBitset::new(Arc::new(mmap), 0).unwrap();

    c.bench_function("hi_sparse_bitset iter", |b| b.iter(|| iteration(black_box(&set))));
    c.bench_function("mmap iter", |b| b.iter(|| mmap_iteration(black_box(&mmap_set))));
}

criterion_group!(benches_iter, bench_iter);
criterion_main!(benches_iter);