/// Benchmarks for the byte shuffle filter implementation.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use s3_active_storage::filters::shuffle;

fn criterion_benchmark(c: &mut Criterion) {
    for size_k in [64, 256, 1024] {
        let size = size_k * 1024;
        let data: Vec<u8> = (0_u32..size)
            .map(|i| u8::try_from(i % 256).unwrap())
            .collect::<Vec<u8>>();
        let bytes = data.into();
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
