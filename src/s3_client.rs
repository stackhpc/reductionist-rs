/// This module provides a simplified S3 client that supports downloading objects.
/// It attempts to hide the complexities of working with the AWS SDK for S3.
use aws_config::{self, meta::region::RegionProviderChain};
use aws_credential_types::Credentials;
use aws_sdk_s3::Client;
use aws_types::region::Region;
use axum::body::Bytes;
use url::Url;

/// S3 client object.
pub struct S3Client {
    /// Underlying AWS SDK S3 client object.
    client: Client,
}

impl S3Client {
    /// Creates an S3Client object
    ///
    /// # Arguments
    ///
    /// * `url`: Object storage API URL
    /// * `username`: Object storage account username
    /// * `password`: Object storage account password
    pub async fn new(url: &Url, username: &str, password: &str) -> Self {
        let credentials = Credentials::from_keys(username, password, None);
        let region = RegionProviderChain::default_provider().or_else(Region::new("us-east-1"));
        let config = aws_config::from_env().region(region).load().await;
        let s3_config = aws_sdk_s3::config::Builder::from(&config)
            .credentials_provider(credentials)
            .endpoint_url(url.to_string())
            .force_path_style(true)
            .build();
        let client = Client::from_conf(s3_config);
        Self { client }
    }

    /// Downloads an object from object storage and returns the data as Bytes
    ///
    /// # Arguments
    ///
    /// * `bucket`: Name of the bucket
    /// * `key`: Name of the object in the bucket
    /// * `range`: Optional byte range
    pub async fn download_object(
        self: &S3Client,
        bucket: &str,
        key: &str,
        range: Option<String>,
    ) -> Bytes {
        let response = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .set_range(range)
            .send()
            .await
            .unwrap();
        let data = response.body.collect().await;
        // TODO: Provide a streaming response.
        data.unwrap().into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[tokio::test]
    async fn new() {
        let url = Url::parse("http://example.com").unwrap();
        S3Client::new(&url, "user", "password").await;
    }
}
