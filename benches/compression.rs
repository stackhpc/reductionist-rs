/// Benchmarks for the byte shuffle filter implementation.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use reductionist::compression;
use reductionist::models;

use axum::body::Bytes;
use blusc::{blosc2_compress, BLOSC2_MAX_OVERHEAD, BLOSC_SHUFFLE};
use flate2::read::{GzEncoder, ZlibEncoder};
use flate2::Compression;
use std::io::Read;
// Bring trait into scope to use as_bytes method.
use zerocopy::AsBytes;

fn compress_blosc(data: &[u8]) -> Bytes {
    // Adapted from blosc documentation.
    let mut compressed = vec![0u8; data.len() + BLOSC2_MAX_OVERHEAD];
    let cbytes = blosc2_compress(5, BLOSC_SHUFFLE as i32, 4, data, &mut compressed);
    // Validate that compression succeeded and the returned size is usable.
    if cbytes <= 0 {
        panic!("blosc2_compress failed with return code {cbytes}");
    }
    if cbytes as usize > compressed.len() {
        panic!(
            "blosc2_compress returned size {cbytes} exceeding buffer length {}",
            compressed.len()
        );
    }
    compressed.truncate(cbytes as usize);
    compressed.into()
}

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
        models::Compression::Blosc2 => compress_blosc(data),
        models::Compression::Gzip => compress_gzip(data),
        models::Compression::Zlib => compress_zlib(data),
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let compression_algs = [
        (models::Compression::Blosc2, "blosc2"),
        (models::Compression::Gzip, "gzip"),
        (models::Compression::Zlib, "zlib"),
    ];
    for (compression, name) in compression_algs {
        for size_k in [64, 256, 1024] {
            let size = size_k * 1024;
            let data: Vec<i64> = (0_i64..size).map(|i| i % 256).collect::<Vec<i64>>();
            let bytes = Bytes::copy_from_slice(data.as_bytes());
            let compressed = compress(compression, &bytes);
            let name = format!("decompress({name}, {size})");
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
