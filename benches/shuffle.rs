/// Benchmarks for the byte shuffle filter implementations.
use axum::body::Bytes;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use reductionist::filters::shuffle;
use reductionist::filters::shuffle_simd;
// Bring trait into scope to use as_bytes method.
use zerocopy::AsBytes;

fn deshuffle(shuffle_alg: &str, data: &Bytes, element_size: usize) -> Bytes {
    match shuffle_alg {
        "shuffle" => shuffle::deshuffle(data, element_size),
        "shuffle_simd" => shuffle_simd::deshuffle(data, element_size),
        _ => panic!("Unknown shuffle algorithm: {shuffle_alg}"),
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let shuffle_algs = ["shuffle", "shuffle_simd"];
    for size_k in [64, 256, 1024, 4096] {
        let size = size_k * 1024;
        let data: Vec<i64> = (0_i64..size).map(|i| i % 256).collect::<Vec<i64>>();
        let bytes = Bytes::copy_from_slice(data.as_bytes());
        for element_size in [2, 4, 8] {
            for shuffle_alg in shuffle_algs {
                let name = format!("de{shuffle_alg}({size}, {element_size})");
                c.bench_function(&name, |b| {
                    b.iter(|| {
                        deshuffle(shuffle_alg, black_box(&bytes), element_size);
                    })
                });
            }
        }
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
