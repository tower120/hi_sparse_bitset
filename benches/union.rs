use std::ops::ControlFlow;
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput, PlotConfiguration, AxisScale};
use criterion::measurement::Measurement;
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
    const SETS: usize = 3;

    fn generate_data(size: usize, index_mul: usize, sets: usize) -> (Vec<HiSparseBitset>, Vec<hibitset::BitSet>){
        let mut random_indices: Vec<Vec<usize>> = Default::default();
        for s in 0..sets{
            let offset = s * (size - size/5) * index_mul;
            random_indices.push(Default::default());
            let random_indices = random_indices.last_mut().unwrap();
            for i in 0..size{
                random_indices.push(offset + i*index_mul);
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

        (hi_sparse_sets, hibitsets)
    }

    fn bench<'a, M, P, I, F, R>(
        group: &mut criterion::BenchmarkGroup<'a, M>,
        case: &str,
        param: P,
        input: &I,
        f: F
    ) where
        M: Measurement,
        P: std::fmt::Display,
        I: ?Sized,
        F: Fn(&I) -> R,
    {
        group.bench_with_input(
            BenchmarkId::new(case, param),
            input,
            |b, i| b.iter(|| f(black_box(i)))
        );
    }

    fn do_bench<'a, M: Measurement>(group: &mut criterion::BenchmarkGroup<'a, M>, index_mul: usize){
        let datas = [
            (100, generate_data(100, index_mul, SETS)),
            (1000, generate_data(1000, index_mul, SETS)),
            (4000, generate_data(4000, index_mul, SETS)),
        ];

        for (name, (hi_sparse_sets, hibitsets)) in &datas {
            let hi_sparse_sets = hi_sparse_sets.as_slice();
            let hibitsets = hibitsets.as_slice();

            // ---- REDUCE ----
            bench(group, "hi_sparse_bitset_reduce_or_simple_block_iter", name, hi_sparse_sets, hi_sparse_bitset_reduce_or_simple_block_iter);
            bench(group, "hi_sparse_bitset_reduce_or_caching_block_iter", name, hi_sparse_sets, hi_sparse_bitset_reduce_or_caching_block_iter);
            bench(group, "hi_sparse_bitset_reduce_or_simple_iter", name, hi_sparse_sets, hi_sparse_bitset_reduce_or_simple_iter);
            bench(group, "hi_sparse_bitset_reduce_or_caching_iter", name, hi_sparse_sets, hi_sparse_bitset_reduce_or_caching_iter);

            // ---- OP ----
            bench(group, "hi_sparse_bitset_op_or_simple_block_iter", name, hi_sparse_sets, hi_sparse_bitset_op_or_simple_block_iter);
            bench(group, "hi_sparse_bitset_op_or_caching_block_iter", name, hi_sparse_sets, hi_sparse_bitset_op_or_caching_block_iter);
            bench(group, "hi_sparse_bitset_op_or_simple_iter", name, hi_sparse_sets, hi_sparse_bitset_op_or_simple_iter);
            bench(group, "hi_sparse_bitset_op_or_caching_iter", name, hi_sparse_sets, hi_sparse_bitset_op_or_caching_iter);

            // ---- Third party ----
            bench(group, "hibitset_union", name, hibitsets, hibitset_union);
        }
    }

    {
        let mut group = c.benchmark_group("Union - index step 20");
        group.plot_config(
            PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)
        );
        do_bench(&mut group, 20);
    }
    {
        let mut group = c.benchmark_group("Union - index step 200");
        group.plot_config(
            PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)
        );
        do_bench(&mut group, 200);
    }
}

criterion_group!(benches_iter, bench_iter);
criterion_main!(benches_iter);