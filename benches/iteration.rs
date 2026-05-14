use criterion::{black_box, Criterion, criterion_group, criterion_main};
use hi_sparse_bitset::{BitSetInterface, ImmutableBitset};
use memmap2::Mmap;

type Conf = hi_sparse_bitset::config::_256bit;
type HiSparseBitset = hi_sparse_bitset::BitSet<Conf>;
type MMapBitset<'a> = hi_sparse_bitset::DirectBitset<Conf, &'a[u8], true>;

fn iteration(set: impl BitSetInterface<Conf=Conf>) -> u64{
    let mut s = 0;
    for data in set.block_iter(){
        s += data.len() as u64;
    }
    s
}

pub fn bench_iter(c: &mut Criterion) {
    let mut set: HiSparseBitset = Default::default();
    for i in 0..4_000_000{
        set.insert(i*4);
    }

    let mut file = tempfile::tempfile().unwrap();
    set.serialize(&mut file).unwrap();

    let mmap = unsafe { Mmap::map(&file).unwrap()  };
    let mmap_set = MMapBitset::new(&*mmap, 0).unwrap();

    let im: ImmutableBitset<Conf> = (&set).into();

    c.bench_function("hi_sparse_bitset iter", |b| b.iter(|| iteration(black_box(&set))));
    c.bench_function("im iter", |b| b.iter(|| iteration(black_box(&im))));
    c.bench_function("mmap iter", |b| b.iter(|| iteration(black_box(&mmap_set))));
}

criterion_group!(benches_iter, bench_iter);
criterion_main!(benches_iter);