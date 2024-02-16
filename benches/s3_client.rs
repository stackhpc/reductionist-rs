/// Benchmarks for S3 client implementation.
use aws_credential_types::Credentials;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use aws_types::region::Region;
use axum::body::Bytes;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use reductionist::resource_manager::ResourceManager;
use reductionist::s3_client::{S3Client, S3ClientMap, S3Credentials};
use url::Url;
// Bring trait into scope to use as_bytes method.
use zerocopy::AsBytes;

async fn upload(url: &Url, username: &str, password: &str, bucket: &str, key: &str, bytes: Bytes) {
    let credentials = Credentials::from_keys(username, password, None);
    let region = Region::new("us-east-1");
    let s3_config = aws_sdk_s3::Config::builder()
        .credentials_provider(credentials)
        .region(Some(region))
        .endpoint_url(url.to_string())
        .force_path_style(true)
        .build();
    let client = Client::from_conf(s3_config);
    let head = client.head_bucket().bucket(bucket).send().await;
    if head.is_err() {
        client.create_bucket().bucket(bucket).send().await.unwrap();
    };
    let body = ByteStream::from(bytes);
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(body)
        .send()
        .await
        .unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    let url = Url::parse("http://localhost:9000").unwrap();
    let username = "minioadmin";
    let password = "minioadmin";
    let credentials = S3Credentials::access_key(username, password);
    let bucket = "s3-client-bench";
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let map = S3ClientMap::new();
    let resource_manager = ResourceManager::new(None, None, None);
    for size_k in [64, 256, 1024] {
        let size: isize = size_k * 1024;
        let data: Vec<u32> = (0_u32..(size as u32)).collect::<Vec<u32>>();
        let key = format!("data-{}", size);
        let bytes = Bytes::copy_from_slice(data.as_bytes());
        runtime.block_on(upload(&url, username, password, bucket, &key, bytes));
        let name = format!("s3_client({})", size);
        c.bench_function(&name, |b| {
            b.to_async(&runtime).iter(|| async {
                let client = S3Client::new(&url, credentials.clone()).await;
                client
                    .download_object(black_box(bucket), &key, None, &resource_manager, &mut None)
                    .await
                    .unwrap();
            })
        });
        let name = format!("s3_client_map({})", size);
        c.bench_function(&name, |b| {
            b.to_async(&runtime).iter(|| async {
                let client = map.get(&url, credentials.clone()).await;
                client
                    .download_object(black_box(bucket), &key, None, &resource_manager, &mut None)
                    .await
                    .unwrap();
            })
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
