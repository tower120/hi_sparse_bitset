use std::ops::ControlFlow;
use std::collections::HashSet;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hibitset::BitSetLike;
use hi_sparse_bitset::{BitSet, IConfig, iter, reduce};
use hi_sparse_bitset::binary_op::BitAndOp;
use hi_sparse_bitset::iter::{BlockIterator, CachingBlockIter, CachingIndexIter, SimpleBlockIter, SimpleIndexIter};
use ControlFlow::*;

// TODO: consider bench different Cache modes instead.

// ---- REDUCE -----
// === Block iter ===
fn hi_sparse_bitset_reduce_and_simple_block_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let reduce = reduce(BitAndOp, sets.iter()).unwrap();
    SimpleBlockIter::new(reduce).count()
}

fn hi_sparse_bitset_reduce_and_caching_block_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let reduce = reduce(BitAndOp, sets.iter()).unwrap();
    CachingBlockIter::new(reduce).count()
}

// === Traverse ===
fn hi_sparse_bitset_reduce_and_simple_traverse<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let reduce = reduce(BitAndOp, sets.iter()).unwrap();

    let mut counter = 0;
    for block in SimpleBlockIter::new(reduce) {
        block.traverse(|_|{
            counter += 1;
            Continue(())
        });
    }
    counter
}

fn hi_sparse_bitset_reduce_and_caching_traverse<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let reduce = reduce(BitAndOp, sets.iter()).unwrap();

    let mut counter = 0;
    for block in CachingBlockIter::new(reduce) {
        block.traverse(|_|{
            counter += 1;
            Continue(())
        });
    }
    counter
}

// === Iter ===
fn hi_sparse_bitset_reduce_and_simple_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let reduce = reduce(BitAndOp, sets.iter()).unwrap();
    SimpleIndexIter::new(SimpleBlockIter::new(reduce)).count()
}

fn hi_sparse_bitset_reduce_and_caching_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let reduce = reduce(BitAndOp, sets.iter()).unwrap();
    CachingIndexIter::new(CachingBlockIter::new(reduce)).count()
}


// ---- OP -----
// === Block iter ===
fn hi_sparse_bitset_op_and_simple_block_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize{
    let intersection = &sets[0] & &sets[1] & &sets[2] & &sets[3] & &sets[4];
    SimpleBlockIter::new(intersection).count()
}

fn hi_sparse_bitset_op_and_caching_block_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize{
    let intersection = &sets[0] & &sets[1] & &sets[2] & &sets[3] & &sets[4];
    CachingBlockIter::new(intersection).count()
}

// === Traverse ===
fn hi_sparse_bitset_op_and_simple_traverse<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let intersection = &sets[0] & &sets[1] & &sets[2] & &sets[3] & &sets[4];

    let mut counter = 0;
    for block in SimpleBlockIter::new(intersection) {
        block.traverse(|_|{
            counter += 1;
            Continue(())
        });
    }
    counter
}

fn hi_sparse_bitset_op_and_caching_traverse<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let intersection = &sets[0] & &sets[1] & &sets[2] & &sets[3] & &sets[4];

    let mut counter = 0;
    for block in CachingBlockIter::new(intersection) {
        block.traverse(|_|{
            counter += 1;
            Continue(())
        });
    }
    counter
}

// === Iter ===
fn hi_sparse_bitset_op_and_simple_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let intersection = &sets[0] & &sets[1] & &sets[2] & &sets[3] & &sets[4];
    SimpleIndexIter::new(SimpleBlockIter::new(intersection)).count()
}

fn hi_sparse_bitset_op_and_caching_iter<Conf: IConfig>(sets: &[BitSet<Conf>]) -> usize {
    let intersection = &sets[0] & &sets[1] & &sets[2] & &sets[3] & &sets[4];
    CachingIndexIter::new(CachingBlockIter::new(intersection)).count()
}

fn hibitset_intersection(sets: &[hibitset::BitSet]) -> usize{
    // Looks like this is the best possible way of doing multi intersection with hibitset.
    let intersection = &sets[0] & &sets[1] & &sets[2] & &sets[3] & &sets[4];

    let mut counter = 0;
    for _ in intersection{
        counter += 1;
    }
    counter
}

fn hashset_intersection(sets: &Vec<HashSet<usize>>) -> usize {
    let mut counter = 0;

    let (first, other) = sets.split_first().unwrap();
    for i in first.iter(){
        let mut intersects = true;
        for o in other{
            if !o.contains(i){
                intersects = false;
                break;
            }
        }

        if intersects{
            counter += 1;
        }
    }

    counter
}

// TODO : Bench with worst-case parameters
pub fn bench_iter(c: &mut Criterion) {
    type HiSparseBitset = hi_sparse_bitset::BitSet<hi_sparse_bitset::configs::_128bit>;
    //type HiSparseBitset = hi_sparse_bitset::HiSparseBitset<hi_sparse_bitset::configs::u64s>;
    const SIZE: usize = 10000;
    const INDEX_MUL: usize = 20;
    const SETS: usize = 5;

    let mut random_indices = [[0; SIZE]; SETS];
    for s in 0..SETS{
        for i in 0..SIZE{
            random_indices[s][i] = i*INDEX_MUL;
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

    let mut hash_sets = Vec::new();
    for set_indices in &random_indices{
        let mut set: HashSet<usize> = HashSet::default();
        for &index in set_indices.iter(){
            set.insert(index);
        }
        hash_sets.push(set);
    }


    // ---- REDUCE -----
    // === Block iter ===
    c.bench_function("hi_sparse_bitset_reduce_and_simple_block_iter", |b| b.iter(|| hi_sparse_bitset_reduce_and_simple_block_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_reduce_and_caching_block_iter", |b| b.iter(|| hi_sparse_bitset_reduce_and_caching_block_iter(black_box(&hi_sparse_sets))));
    // === Traverse ===
    c.bench_function("hi_sparse_bitset_reduce_and_simple_traverse", |b| b.iter(|| hi_sparse_bitset_reduce_and_simple_traverse(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_reduce_and_caching_traverse", |b| b.iter(|| hi_sparse_bitset_reduce_and_caching_traverse(black_box(&hi_sparse_sets))));
    // === Iter ===
    c.bench_function("hi_sparse_bitset_reduce_and_simple_iter", |b| b.iter(|| hi_sparse_bitset_reduce_and_simple_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_reduce_and_caching_iter", |b| b.iter(|| hi_sparse_bitset_reduce_and_caching_iter(black_box(&hi_sparse_sets))));

    // ---- OP -----
    // === Block iter ===
    c.bench_function("hi_sparse_bitset_op_and_simple_block_iter", |b| b.iter(|| hi_sparse_bitset_op_and_simple_block_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_op_and_caching_block_iter", |b| b.iter(|| hi_sparse_bitset_op_and_caching_block_iter(black_box(&hi_sparse_sets))));
    // === Traverse ===
    c.bench_function("hi_sparse_bitset_op_and_simple_traverse", |b| b.iter(|| hi_sparse_bitset_op_and_simple_traverse(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_op_and_caching_traverse", |b| b.iter(|| hi_sparse_bitset_op_and_caching_traverse(black_box(&hi_sparse_sets))));
    // === Iter ===
    c.bench_function("hi_sparse_bitset_op_and_simple_iter", |b| b.iter(|| hi_sparse_bitset_op_and_simple_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_op_and_caching_iter", |b| b.iter(|| hi_sparse_bitset_op_and_caching_iter(black_box(&hi_sparse_sets))));

    // ---- Third party ----
    c.bench_function("hibitset_intersection", |b| b.iter(|| hibitset_intersection(black_box(&hibitsets))));
    c.bench_function("hashset_intersection",  |b| b.iter(|| hashset_intersection(black_box(&hash_sets))));
}

criterion_group!(benches_iter, bench_iter);
criterion_main!(benches_iter);