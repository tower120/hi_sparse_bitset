#![allow(unused_imports)]

mod common;

use std::ops::ControlFlow;
use std::collections::HashSet;
use criterion::{AxisScale, Criterion, criterion_group, criterion_main, PlotConfiguration};
use hi_sparse_bitset::{BitSet, BitSetInterface, reduce, traverse_from, traverse_index_from};
use hi_sparse_bitset::binary_op::BitAndOp;
use hi_sparse_bitset::iter::{BlockIterator, BlockCursor, IndexCursor, SimpleBlockIter, SimpleIndexIter};
use ControlFlow::*;
use criterion::measurement::Measurement;
use roaring::RoaringBitmap;
use hi_sparse_bitset::config::Config;
use crate::common::bench;

// TODO: consider bench different Cache modes instead.

// ---- REDUCE -----
// === Block iter ===
fn hi_sparse_bitset_reduce_and_simple_block_iter<Conf: Config>(sets: &[BitSet<Conf>]) -> usize {
    let reduce = reduce(BitAndOp, sets.iter()).unwrap();
    SimpleBlockIter::new(reduce).count()
}

fn hi_sparse_bitset_reduce_and_caching_block_iter<Conf: Config>(sets: &[BitSet<Conf>]) -> usize {
    let reduce = reduce(BitAndOp, sets.iter()).unwrap();
    reduce.into_block_iter().count()
}

// === Traverse ===
fn hi_sparse_bitset_reduce_and_simple_traverse<Conf: Config>(sets: &[BitSet<Conf>]) -> usize {
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

fn hi_sparse_bitset_reduce_and_caching_traverse<Conf: Config>(sets: &[BitSet<Conf>]) -> usize {
    let reduce = reduce(BitAndOp, sets.iter()).unwrap();

    let mut counter = 0;
    
/*    traverse_from(&reduce, BlockIterCursor::default(), |block|{
        block.traverse(|_|{
            counter += 1;
            Continue(())
        })
    });*/
    
    /* traverse_index_from(&reduce, IndexIterCursor::default(), |_|{
        counter += 1;
        Continue(())
    }); */

/*     reduce.block_iter().traverse(|block|{
        block.traverse(|_|{
            counter += 1;
            Continue(())    
        })
    }); */

    reduce.iter().traverse(|_|{
        counter += 1;
        Continue(())    
    });    
    
    /*reduce.traverse(|_|{
        counter += 1;
        Continue(())
    });*/
    counter
}

// === Iter ===
fn hi_sparse_bitset_reduce_and_simple_iter<Conf: Config>(sets: &[BitSet<Conf>]) -> usize {
    let reduce = reduce(BitAndOp, sets.iter()).unwrap();
    SimpleIndexIter::new(SimpleBlockIter::new(reduce)).count()
}

fn hi_sparse_bitset_reduce_and_caching_iter<Conf: Config>(sets: &[BitSet<Conf>]) -> usize {
    let reduce = reduce(BitAndOp, sets.iter()).unwrap();
    reduce.into_iter().count()
}


// ---- OP -----
// === Block iter ===
fn hi_sparse_bitset_op_and_simple_block_iter<Conf: Config>(sets: &[BitSet<Conf>]) -> usize{
    let intersection = &sets[0] & &sets[1] & &sets[2] & &sets[3] & &sets[4];
    SimpleBlockIter::new(intersection).count()
}

fn hi_sparse_bitset_op_and_caching_block_iter<Conf: Config>(sets: &[BitSet<Conf>]) -> usize{
    let intersection = &sets[0] & &sets[1] & &sets[2] & &sets[3] & &sets[4];
    intersection.into_block_iter().count()
}

// === Traverse ===
fn hi_sparse_bitset_op_and_simple_traverse<Conf: Config>(sets: &[BitSet<Conf>]) -> usize {
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

fn hi_sparse_bitset_op_and_caching_traverse<Conf: Config>(sets: &[BitSet<Conf>]) -> usize {
    let intersection = &sets[0] & &sets[1] & &sets[2] & &sets[3] & &sets[4];

    let mut counter = 0;
    intersection.traverse(|_|{
        counter += 1;
        Continue(())
    });
    counter
}

// === Iter ===
fn hi_sparse_bitset_op_and_simple_iter<Conf: Config>(sets: &[BitSet<Conf>]) -> usize {
    let intersection = &sets[0] & &sets[1] & &sets[2] & &sets[3] & &sets[4];
    SimpleIndexIter::new(SimpleBlockIter::new(intersection)).count()
}

fn hi_sparse_bitset_op_and_caching_iter<Conf: Config>(sets: &[BitSet<Conf>]) -> usize {
    let intersection = &sets[0] & &sets[1] & &sets[2] & &sets[3] & &sets[4];
    intersection.into_iter().count()
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

fn hashset_intersection(sets: &[HashSet<usize>]) -> usize {
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

fn roaring_intersection(roarings: &[RoaringBitmap]) -> usize{
    // There is no equivalent in RoaringBitmap for multiple set intersection.
    // Constructing a new one for each intersection would not be fare for the underlying algorithm.
    // This is probably the closest one computation-wise, since all input sets actually fully intersects.

    let len =
    roarings[0].intersection_len(&roarings[1]) +
    roarings[0].intersection_len(&roarings[2]) +
    roarings[0].intersection_len(&roarings[3]) +
    roarings[0].intersection_len(&roarings[4]);

    len as usize
}

pub fn bench_iter(c: &mut Criterion) {
    type HiSparseBitset = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_128bit>;
    const SETS: usize = 5;

    fn generate_data(size: usize, index_mul: usize, sets: usize)
        -> (Vec<HiSparseBitset>, Vec<hibitset::BitSet>, Vec<HashSet<usize>>, Vec<RoaringBitmap>)
    {
        let mut random_indices: Vec<Vec<usize>> = Default::default();
        for _ in 0..sets{
            random_indices.push(Default::default());
            let random_indices = random_indices.last_mut().unwrap();
            for i in 0..size{
                random_indices.push(i*index_mul);
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

        let mut roarings = Vec::new();
        for set_indices in &random_indices{
            let mut set: RoaringBitmap = Default::default();
            for &index in set_indices.iter(){
                set.insert(index as u32);
            }
            roarings.push(set);
        }

        (hi_sparse_sets, hibitsets, hash_sets, roarings)
    }

    fn do_bench<'a, M: Measurement>(group: &mut criterion::BenchmarkGroup<'a, M>, index_mul: usize){
        let datas = [
            (100, generate_data(100, index_mul, SETS)),
            (1000, generate_data(1000, index_mul, SETS)),
            (4000, generate_data(4000, index_mul, SETS)),
        ];

        for (name, (hi_sparse_sets, hibitsets, hash_sets, roarings)) in &datas {
            let hi_sparse_sets = hi_sparse_sets.as_slice();

            // ---- REDUCE ----
            // === Block iter ===
            bench(group, "hi_sparse_bitset_reduce_and_simple_block_iter", name, hi_sparse_sets, hi_sparse_bitset_reduce_and_simple_block_iter);
            bench(group, "hi_sparse_bitset_reduce_and_caching_block_iter", name, hi_sparse_sets, hi_sparse_bitset_reduce_and_caching_block_iter);
            // === Traverse ===
            bench(group, "hi_sparse_bitset_reduce_and_simple_traverse", name, hi_sparse_sets, hi_sparse_bitset_reduce_and_simple_traverse);
            bench(group, "hi_sparse_bitset_reduce_and_caching_traverse", name, hi_sparse_sets, hi_sparse_bitset_reduce_and_caching_traverse);
            // === Iter ===
            bench(group, "hi_sparse_bitset_reduce_and_simple_iter", name, hi_sparse_sets, hi_sparse_bitset_reduce_and_simple_iter);
            bench(group, "hi_sparse_bitset_reduce_and_caching_iter", name, hi_sparse_sets, hi_sparse_bitset_reduce_and_caching_iter);

            // ---- OP ----
            // === Block iter ===
            bench(group, "hi_sparse_bitset_op_and_simple_block_iter", name, hi_sparse_sets, hi_sparse_bitset_op_and_simple_block_iter);
            bench(group, "hi_sparse_bitset_op_and_caching_block_iter", name, hi_sparse_sets, hi_sparse_bitset_op_and_caching_block_iter);
            // === Traverse ===
            bench(group, "hi_sparse_bitset_op_and_simple_traverse", name, hi_sparse_sets, hi_sparse_bitset_op_and_simple_traverse);
            bench(group, "hi_sparse_bitset_op_and_caching_traverse", name, hi_sparse_sets, hi_sparse_bitset_op_and_caching_traverse);
            // === Iter ===
            bench(group, "hi_sparse_bitset_op_and_simple_iter", name, hi_sparse_sets, hi_sparse_bitset_op_and_simple_iter);
            bench(group, "hi_sparse_bitset_op_and_caching_iter", name, hi_sparse_sets, hi_sparse_bitset_op_and_caching_iter);

            // ---- Third party ----
            bench(group, "hibitset_intersection", name, hibitsets.as_slice(), hibitset_intersection);
            bench(group, "hashset_intersection", name, hash_sets.as_slice(), hashset_intersection);
            bench(group, "roaring_intersection", name, roarings.as_slice(), roaring_intersection);
        }
    }

    {
        let mut group = c.benchmark_group("Intersection - index step 20");
        /*group.plot_config(
            PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)
        );*/
        do_bench(&mut group, 20);
    }
    {
        let mut group = c.benchmark_group("Intersection - index step 200");
        /*group.plot_config(
            PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)
        );*/
        do_bench(&mut group, 200);
    }
}

criterion_group!(benches_iter, bench_iter);
criterion_main!(benches_iter);