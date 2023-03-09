/// This module provides a simplified S3 client that supports downloading objects.
/// It attempts to hide the complexities of working with the AWS SDK for S3.
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
        let region = Region::new("us-east-1");
        let s3_config = aws_sdk_s3::Config::builder() //&config)
            .credentials_provider(credentials)
            .region(Some(region))
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
        // TODO: Provide a streaming response.
        let response = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .set_range(range)
            .send()
            .await
            .unwrap();
        let content_length = response.content_length();
        // The data returned by the S3 client does not have any alignment guarantees. In order to
        // reinterpret the data as an array of numbers with a higher alignment than 1, we need to
        // return the data in Bytes object in which the underlying data has a higher alignment.
        // For now we're hard-coding an alignment of 8 bytes, although this should depend on the
        // data type, and potentially whether there are any SIMD requirements.
        // FIXME: The current method is rather inefficient, involving copying the data at least
        // twice. This is functional, but should be revisited.

        // Read all data into memory as an AggregatedBytes.
        let data = response.body.collect().await;
        // Create an 8-byte aligned Vec<u8>.
        let mut buf = maligned::align_first::<u8, maligned::A8>(content_length as usize);
        // Copy the data into an unaligned Vec<u8>.
        let mut vec = data.unwrap().to_vec();
        // Copy the data into the aligned Vec<u8>.
        buf.append(&mut vec);
        // Return as Bytes.
        buf.into()
    }
}

/// Return an optional byte range string based on the offset and size.
///
/// The returned string is compatible with the HTTP Range header.
///
/// # Arguments
///
/// * `offset`: Optional offset of data in bytes
/// * `size`: Optional size of data in bytes
pub fn get_range(offset: Option<usize>, size: Option<usize>) -> Option<String> {
    match (offset, size) {
        (offset, Some(size)) => {
            // Default offset to 0.
            let offset = offset.unwrap_or(0);
            // Range-end is inclusive.
            let end = offset + size - 1;
            Some(format!("bytes={}-{}", offset, end))
        }
        (Some(offset), None) => Some(format!("bytes={}-", offset)),
        _ => None,
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

    #[test]
    fn get_range_none() {
        assert_eq!(None, get_range(None, None));
    }

    #[test]
    fn get_range_both() {
        assert_eq!(Some("bytes=1-2".to_string()), get_range(Some(1), Some(2)));
    }

    #[test]
    fn get_range_offset() {
        assert_eq!(Some("bytes=1-".to_string()), get_range(Some(1), None));
    }

    #[test]
    fn get_range_size() {
        assert_eq!(Some("bytes=0-1".to_string()), get_range(None, Some(2)));
    }
}
