/// Benchmarks for the byte shuffle filter implementation.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use reductionist::compression;
use reductionist::models;

use axum::body::Bytes;
use flate2::read::{GzEncoder, ZlibEncoder};
use flate2::Compression;
use std::io::Read;
// Bring trait into scope to use as_bytes method.
use zerocopy::AsBytes;

fn compress_gzip(data: &[u8]) -> Bytes {
    // Adapated from flate2 documentation.
    let mut result = Vec::<u8>::new();
    let mut deflater = GzEncoder::new(data, Compression::fast());
    deflater.read_to_end(&mut result).unwrap();
    result.into()
}

fn compress_zlib(data: &[u8]) -> Bytes {
    // Adapated from flate2 documentation.
    let mut result = Vec::<u8>::new();
    let mut deflater = ZlibEncoder::new(data, Compression::fast());
    deflater.read_to_end(&mut result).unwrap();
    result.into()
}

fn compress(compression: models::Compression, data: &[u8]) -> Bytes {
    match compression {
        models::Compression::Gzip => compress_gzip(data),
        models::Compression::Zlib => compress_zlib(data),
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let compression_algs = [
        (models::Compression::Gzip, "gzip"),
        (models::Compression::Zlib, "zlib"),
    ];
    for (compression, name) in compression_algs {
        for size_k in [64, 256, 1024] {
            let size = size_k * 1024;
            let data: Vec<i64> = (0_i64..size).map(|i| i % 256).collect::<Vec<i64>>();
            let bytes = Bytes::copy_from_slice(data.as_bytes());
            let compressed = compress(compression, &bytes);
            let name = format!("decompress({}, {})", name, size);
            c.bench_function(&name, |b| {
                b.iter(|| {
                    compression::decompress(compression, black_box(&compressed)).unwrap();
                })
            });
        }
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
