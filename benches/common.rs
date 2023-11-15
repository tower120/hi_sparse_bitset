use criterion::{BenchmarkId, black_box};
use criterion::measurement::Measurement;

pub fn bench<'a, M, P, I, F, R>(
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