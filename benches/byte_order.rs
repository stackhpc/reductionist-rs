/// Benchmarks for the byte order reversal implementation.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use reductionist::array::{build_array_mut_from_shape, get_shape, reverse_array_byte_order};
use reductionist::models::{DType, RequestData, Slice};
use url::Url;

fn get_test_request_data() -> RequestData {
    RequestData {
        source: Url::parse("http://example.com").unwrap(),
        bucket: "bar".to_string(),
        object: "baz".to_string(),
        dtype: DType::Int32,
        byte_order: None,
        offset: None,
        size: None,
        shape: None,
        axis: reductionist::models::ReductionAxes::All,
        order: None,
        selection: None,
        compression: None,
        filters: None,
        missing: None,
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    for size_k in [64, 256, 1024] {
        let size: isize = size_k * 1024;
        let mut data: Vec<u32> = (0_u32..(size as u32)).collect::<Vec<u32>>();
        let mut request_data = get_test_request_data();
        request_data.dtype = DType::Uint32;
        let shape = get_shape(data.len(), &request_data);
        let mut array = build_array_mut_from_shape(shape, &mut data).unwrap();
        for selection in [None, Some(vec![Slice::new(size / 4, size / 2, 2)])] {
            let name = format!("byte_order({}, {:?})", size, selection);
            c.bench_function(&name, |b| {
                b.iter(|| {
                    reverse_array_byte_order(black_box(&mut array), &selection);
                })
            });
        }
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
