use std::ops::ControlFlow;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hibitset::BitSetLike;
use hi_sparse_bitset::{BitSet, IConfig, reduce};
use hi_sparse_bitset::binary_op::*;
use hi_sparse_bitset::iter::{BlockIterator, CachingBlockIter, CachingIndexIter, SimpleBlockIter, SimpleIndexIter};
use hi_sparse_bitset::BitSetInterface;


// ---- REDUCE ----
fn hi_sparse_bitset_reduce_or_simple_block_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let union = reduce(BitOrOp, sets.iter()).unwrap();
    SimpleBlockIter::new(union).count()
}

fn hi_sparse_bitset_reduce_or_caching_block_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let union = reduce(BitOrOp, sets.iter()).unwrap();
    CachingBlockIter::new(union).count()
}

fn hi_sparse_bitset_reduce_or_simple_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let union = reduce(BitOrOp, sets.iter()).unwrap();
    SimpleIndexIter::new(SimpleBlockIter::new(union)).count()
}

fn hi_sparse_bitset_reduce_or_caching_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let union = reduce(BitOrOp, sets.iter()).unwrap();
    CachingIndexIter::new(CachingBlockIter::new(union)).count()
}


// ---- OP ----
fn hi_sparse_bitset_op_or_simple_block_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let union = &sets[0] | &sets[1] | &sets[2];
    SimpleBlockIter::new(union).count()
}

fn hi_sparse_bitset_op_or_caching_block_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let union = &sets[0] | &sets[1] | &sets[2];
    CachingBlockIter::new(union).count()
}

fn hi_sparse_bitset_op_or_simple_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let union = &sets[0] | &sets[1] | &sets[2];
    SimpleIndexIter::new(SimpleBlockIter::new(union)).count()
}

fn hi_sparse_bitset_op_or_caching_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let union = &sets[0] | &sets[1] | &sets[2];
    CachingIndexIter::new(CachingBlockIter::new(union)).count()
}

// ---- Third party ----
fn hibitset_union(sets: &[hibitset::BitSet]) -> usize{
    // Looks like this is the best possible way of doing multi intersection with hibitset.
    let intersection = &sets[0] | &sets[1] | &sets[2];

    let mut counter = 0;
    for _ in intersection{
        counter += 1;
    }
    counter
}


/// Bench worst case scenario for hibitset and default iter.
/// All sets does not have intersections.
pub fn bench_iter(c: &mut Criterion) {
    type HiSparseBitset = hi_sparse_bitset::BitSet<hi_sparse_bitset::configs::_128bit>;
    const SIZE: usize = 10000;
    const INDEX_MUL: usize = 20;
    const SETS: usize = 3;

    let mut random_indices = [[0; SIZE]; SETS];
    for s in 0..SETS{
        let offset = s * (SIZE - SIZE/5) * INDEX_MUL;
        for i in 0..SIZE{
            random_indices[s][i] = offset + i*INDEX_MUL;
        }
    }

    let mut hi_sparse_sets = Vec::new();
    for set_indices in &random_indices{
        let mut set = HiSparseBitset::default();
        for &index in set_indices.iter(){
            set.insert(index);
        }
        hi_sparse_sets.push(set);
    }

    let mut hibitsets = Vec::new();
    for set_indices in &random_indices{
        let mut set = hibitset::BitSet::default();
        for &index in set_indices.iter(){
            set.add(index as _);
        }
        hibitsets.push(set);
    }

    // ---- REDUCE ----
    c.bench_function("hi_sparse_bitset_reduce_or_simple_block_iter", |b| b.iter(|| hi_sparse_bitset_reduce_or_simple_block_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_reduce_or_caching_block_iter", |b| b.iter(|| hi_sparse_bitset_reduce_or_caching_block_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_reduce_or_simple_iter", |b| b.iter(|| hi_sparse_bitset_reduce_or_simple_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_reduce_or_caching_iter", |b| b.iter(|| hi_sparse_bitset_reduce_or_caching_iter(black_box(&hi_sparse_sets))));

    // ---- OP ----
    c.bench_function("hi_sparse_bitset_op_or_simple_block_iter", |b| b.iter(|| hi_sparse_bitset_op_or_simple_block_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_op_or_caching_block_iter", |b| b.iter(|| hi_sparse_bitset_op_or_caching_block_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_op_or_simple_iter", |b| b.iter(|| hi_sparse_bitset_op_or_simple_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_op_or_caching_iter", |b| b.iter(|| hi_sparse_bitset_op_or_caching_iter(black_box(&hi_sparse_sets))));

    // ---- Third party ----
    c.bench_function("hibitset_union", |b| b.iter(|| hibitset_union(black_box(&hibitsets))));
}

criterion_group!(benches_iter, bench_iter);
criterion_main!(benches_iter);