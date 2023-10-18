//mod utils;

use std::ops::ControlFlow;
use std::collections::HashSet;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hi_sparse_bitset::{HiSparseBitset, IConfig, intersection_blocks, intersection_blocks_traverse};

fn hi_bitset_intersection_traverse<Conf: IConfig>(sets: &Vec<HiSparseBitset<Conf>>) -> usize {
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

fn hi_bitset_intersection_iter<Conf: IConfig>(sets: &Vec<HiSparseBitset<Conf>>) -> usize {
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
    const SIZE: usize = 10000;
    const SETS: usize = 5;

    let mut random_indices = [[0; SIZE]; SETS];
    for s in 0..SETS{
        for i in 0..SIZE{
            random_indices[s][i] = i*60;
        }
    }

    let mut hi_sets = Vec::new();
    for set_indices in &random_indices{
        let mut set = HiSparseBitset::default();
        for &index in set_indices.iter(){
            set.insert(index);
        }
        hi_sets.push(set);
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
    c.bench_function("hi_bitset_intersection_iter", |b| b.iter(|| hi_bitset_intersection_iter(black_box(&hi_sets))));
    c.bench_function("hi_bitset_intersection_traverse", |b| b.iter(|| hi_bitset_intersection_traverse(black_box(&hi_sets))));
    c.bench_function("hashset_intersection",   |b| b.iter(|| hashset_intersection(black_box(&hash_sets))));
}

criterion_group!(benches_iter, bench_iter);
criterion_main!(benches_iter);