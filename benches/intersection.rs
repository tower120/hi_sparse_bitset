use std::ops::ControlFlow;
use std::collections::HashSet;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hi_sparse_bitset::{HiSparseBitset, IConfig, intersection_blocks, intersection_blocks_traverse, reduce_and2};

fn hi_sparse_bitset_intersection_traverse<Conf: IConfig>(sets: &[HiSparseBitset<Conf>]) -> usize {
    use ControlFlow::*;

    let mut counter = 0;
    intersection_blocks_traverse(sets, |block| {
        block.traverse(|_|{
            counter += 1;
            Continue(())
        });     
    } );
    counter
}

fn hi_sparse_bitset_intersection_iter<Conf: IConfig>(sets: &[HiSparseBitset<Conf>]) -> usize {
    use ControlFlow::*;

    let iter = intersection_blocks(sets);

    let mut counter = 0;
    for block in iter {
        block.traverse(|_|{
            counter += 1;
            Continue(())
        });     
    }
    counter
}

fn hi_sparse_bitset_reduce_and_iter<Conf: IConfig>(sets: &[HiSparseBitset<Conf>]) -> usize {
    use ControlFlow::*;

    let iter = reduce_and2(sets.iter()).iter();

    let mut counter = 0;
    for block in iter {
        block.traverse(|_|{
            counter += 1;
            Continue(())
        });
    }
    counter
}

fn hi_sparse_bitset_reduce_and_iter_ext<Conf: IConfig>(sets: &[HiSparseBitset<Conf>]) -> usize {
    use ControlFlow::*;

    let iter = reduce_and2(sets.iter()).iter_ext();

    let mut counter = 0;
    for block in iter {
        block.traverse(|_|{
            counter += 1;
            Continue(())
        });
    }
    counter
}


/*// TODO: This does not bench anything.
fn hi_bitset_intersection_iter_resumable(sets: &Vec<HiSparseBitset>) -> usize {
    use ControlFlow::*;

    let state = IntersectionBlocksState::default();
    let iter = state.resume(sets.iter());
    
    let mut counter = 0;
    for (_, block) in iter {
        SimdOp::traverse_one_indices(block, |_|{
            counter += 1;
            Continue(())
        });     
    }
    counter
}*/

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

pub fn bench_iter(c: &mut Criterion) {
    type HiSparseBitset = hi_sparse_bitset::HiSparseBitset<hi_sparse_bitset::configs::simd_128>;
    //type HiSparseBitset = hi_sparse_bitset::HiSparseBitset<hi_sparse_bitset::configs::u64s>;
    const SIZE: usize = 10000;
    const SETS: usize = 5;

    let mut random_indices = [[0; SIZE]; SETS];
    for s in 0..SETS{
        for i in 0..SIZE{
            random_indices[s][i] = i*25;
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
        let mut set = HashSet::default();
        for &index in set_indices.iter(){
            set.insert(index);
        }
        hash_sets.push(set);
    }

    //c.bench_function("hi_bitset_intersection_iter_resumable", |b| b.iter(|| hi_bitset_intersection_iter_resumable(black_box(&hi_sets))));
    c.bench_function("hi_sparse_bitset_reduce_and_iter", |b| b.iter(|| hi_sparse_bitset_reduce_and_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_reduce_and_iter_ext", |b| b.iter(|| hi_sparse_bitset_reduce_and_iter_ext(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_intersection_iter", |b| b.iter(|| hi_sparse_bitset_intersection_iter(black_box(&hi_sparse_sets))));
    c.bench_function("hi_sparse_bitset_intersection_traverse", |b| b.iter(|| hi_sparse_bitset_intersection_traverse(black_box(&hi_sparse_sets))));
    c.bench_function("hibitset_intersection", |b| b.iter(|| hibitset_intersection(black_box(&hibitsets))));
    c.bench_function("hashset_intersection",   |b| b.iter(|| hashset_intersection(black_box(&hash_sets))));
}

criterion_group!(benches_iter, bench_iter);
criterion_main!(benches_iter);