use std::ops::ControlFlow;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hibitset::BitSetLike;
use hi_sparse_bitset::{HiSparseBitset, IConfig, reduce};
use hi_sparse_bitset::binary_op::*;
use hi_sparse_bitset::iter::{BlockIterator, SimpleBlockIter};
use hi_sparse_bitset::BitSetInterface;

fn hi_sparse_bitset_reduce_or_iter<Conf: IConfig>(sets: &[HiSparseBitset<Conf>]) -> usize {
    use ControlFlow::*;

    let iter = reduce(BitOrOp, sets.iter()).unwrap().iter();

    let mut counter = 0;
    for block in iter {
        block.traverse(|_|{
            counter += 1;
            Continue(())
        });
    }
    counter
}

fn hi_sparse_bitset_reduce_or_iter_index<Conf: IConfig>(sets: &[HiSparseBitset<Conf>]) -> usize {
    reduce(BitOrOp, sets.iter()).unwrap().iter()
        .flat_map(|block|block.iter()).count()
}

fn hi_sparse_bitset_reduce_or_iter_ext3<Conf: IConfig>(sets: &[HiSparseBitset<Conf>]) -> usize {
    use ControlFlow::*;

    let iter = reduce(BitOrOp, sets.iter()).unwrap().iter_ext3();

    let mut counter = 0;
    for block in iter {
        block.traverse(|_|{
            counter += 1;
            Continue(())
        });
    }
    counter
}

fn hi_sparse_bitset_reduce_or_iter_ext3_indices<Conf: IConfig>(sets: &[HiSparseBitset<Conf>]) -> usize {
    reduce(BitOrOp, sets.iter()).unwrap().iter_ext3().as_indices().count()
}

fn hi_sparse_bitset_op_or_iter_ext3<Conf: IConfig>(sets: &[HiSparseBitset<Conf>]) -> usize {
    use ControlFlow::*;

    let operation = &sets[0] | &sets[1] | &sets[2];

    let mut counter = 0;
    for block in operation.block_iter() {
        block.traverse(|_|{
            counter += 1;
            Continue(())
        });
    }
    counter
}

fn hi_sparse_bitset_op_or_index_iter_ext3<Conf: IConfig>(sets: &[HiSparseBitset<Conf>]) -> usize {
    let operation = &sets[0] | &sets[1] | &sets[2];
    operation.block_iter().as_indices().count()
}

fn hi_sparse_bitset_op_or_simple_iter<Conf: IConfig>(sets: &[HiSparseBitset<Conf>]) -> usize {
    use ControlFlow::*;

    let operation = &sets[0] | &sets[1] | &sets[2];

    let mut counter = 0;
    for block in SimpleBlockIter::new(operation) {
        block.traverse(|_|{
            counter += 1;
            Continue(())
        });
    }
    counter
}

fn hi_sparse_bitset_op_or_index_simple_iter<Conf: IConfig>(sets: &[HiSparseBitset<Conf>]) -> usize {
    let operation = &sets[0] | &sets[1] | &sets[2];
    SimpleBlockIter::new(operation).flat_map(|block|block.iter()).count()
}

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
    type HiSparseBitset = hi_sparse_bitset::HiSparseBitset<hi_sparse_bitset::configs::_128bit>;
    const SIZE: usize = 1000;
    const INDEX_MUL: usize = 200;
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

    /*let mut hash_sets = Vec::new();
    for set_indices in &random_indices{
        let mut set = HashSet::default();
        for &index in set_indices.iter(){
            set.insert(index);
        }
        hash_sets.push(set);
    }*/

    c.bench_function("hi_sparse_bitset_reduce_or_iter", |b| b.iter(|| hi_sparse_bitset_reduce_or_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_reduce_or_iter_index", |b| b.iter(|| hi_sparse_bitset_reduce_or_iter_index(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_reduce_or_iter_ext3", |b| b.iter(|| hi_sparse_bitset_reduce_or_iter_ext3(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_reduce_or_iter_ext3_indices", |b| b.iter(|| hi_sparse_bitset_reduce_or_iter_ext3_indices(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_op_or_iter_ext3", |b| b.iter(|| hi_sparse_bitset_op_or_iter_ext3(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_op_or_index_iter_ext3", |b| b.iter(|| hi_sparse_bitset_op_or_index_iter_ext3(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_op_or_simple_iter", |b| b.iter(|| hi_sparse_bitset_op_or_simple_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_op_or_index_simple_iter", |b| b.iter(|| hi_sparse_bitset_op_or_index_simple_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hibitset_union", |b| b.iter(|| hibitset_union(black_box(&hibitsets))));
    //c.bench_function("hashset_intersection",   |b| b.iter(|| hashset_intersection(black_box(&hash_sets))));
}

criterion_group!(benches_iter, bench_iter);
criterion_main!(benches_iter);