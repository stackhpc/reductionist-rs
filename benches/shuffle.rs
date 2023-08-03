/// Benchmarks for the byte shuffle filter implementation.
use axum::body::Bytes;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use reductionist::filters::shuffle;
// Bring trait into scope to use as_bytes method.
use zerocopy::AsBytes;

fn criterion_benchmark(c: &mut Criterion) {
    for size_k in [64, 256, 1024, 4096] {
        let size = size_k * 1024;
        let data: Vec<i64> = (0_i64..size).map(|i| i % 256).collect::<Vec<i64>>();
        let bytes = Bytes::copy_from_slice(data.as_bytes());
        for element_size in [2, 4, 8] {
            let name = format!("deshuffle({}, {})", size, element_size);
            c.bench_function(&name, |b| {
                b.iter(|| {
                    shuffle::deshuffle(black_box(&bytes), element_size);
                })
            });
        }
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
