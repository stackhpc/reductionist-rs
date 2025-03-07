/// Benchmarks for numerical operations.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use reductionist::error::ActiveStorageError;
use reductionist::models::{DType, RequestData, Response};
use reductionist::operation::Operation;
use reductionist::operations;
use reductionist::types::Missing;
use url::Url;
// Bring trait into scope to use as_bytes method.
use zerocopy::AsBytes;

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
        axis: None,
        order: None,
        selection: None,
        compression: None,
        filters: None,
        missing: None,
    }
}

type ExecuteFn = dyn Fn(&RequestData, Vec<u8>) -> Result<Response, ActiveStorageError>;

fn criterion_benchmark(c: &mut Criterion) {
    for size_k in [64, 256, 1024, 4096] {
        let size = size_k * 1024;
        let data: Vec<i64> = (0_i64..size).map(|i| i % 256).collect::<Vec<i64>>();
        let data: Vec<u8> = data.as_bytes().into();
        let missings = vec![
            None,
            Some(Missing::MissingValue(42.into())),
            Some(Missing::MissingValues(vec![42.into()])),
            Some(Missing::ValidMax(128.into())),
            Some(Missing::ValidMin(128.into())),
            Some(Missing::ValidRange(5.into(), 250.into())),
        ];
        let operations: [(&str, Box<ExecuteFn>); 5] = [
            ("count", Box::new(operations::Count::execute)),
            ("max", Box::new(operations::Max::execute)),
            ("min", Box::new(operations::Min::execute)),
            ("select", Box::new(operations::Select::execute)),
            ("sum", Box::new(operations::Sum::execute)),
        ];
        for (op_name, execute) in operations {
            for missing in missings.clone() {
                let name = format!("{}({}, {:?})", op_name, size, missing);
                c.bench_function(&name, |b| {
                    b.iter(|| {
                        let mut request_data = get_test_request_data();
                        request_data.dtype = DType::Int64;
                        request_data.missing.clone_from(&missing);
                        execute(&request_data, black_box(data.clone())).unwrap();
                    })
                });
            }
        }
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
