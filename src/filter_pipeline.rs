//! Compression and filter pipeline.

use crate::compression;
use crate::error::ActiveStorageError;
use crate::models;

use axum::body::Bytes;

/// Returns data after applying a filter pipeline.
///
/// The pipeline is applied in the reverse order to when the data was written.
///
/// # Arguments
///
/// * `request_data`: RequestData object for the request
/// * `data`: Data to apply filter pipeline to.
pub fn filter_pipeline(
    request_data: &models::RequestData,
    data: &Bytes,
) -> Result<Bytes, ActiveStorageError> {
    if let Some(compression) = request_data.compression {
        compression::decompress(compression, data)
    } else {
        Ok(data.clone())
    }
    // TODO: Defilter
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use flate2::read::GzEncoder;
    use flate2::Compression;
    use std::io::Read;

    fn compress_gzip(data: &[u8]) -> Bytes {
        // Adapated from flate2 documentation.
        let mut result = Vec::<u8>::new();
        let mut deflater = GzEncoder::new(data, Compression::fast());
        deflater.read_to_end(&mut result).unwrap();
        result.into()
    }

    #[test]
    fn test_filter_pipeline_noop() {
        let data = [1, 2, 3, 4];
        let bytes = Bytes::copy_from_slice(&data);
        let request_data = test_utils::get_test_request_data();
        let result = filter_pipeline(&request_data, &bytes).unwrap();
        assert_eq!(data.as_ref(), result);
    }

    #[test]
    fn test_filter_pipeline_gzip() {
        let data = [1, 2, 3, 4];
        let bytes = compress_gzip(data.as_ref());
        let mut request_data = test_utils::get_test_request_data();
        request_data.compression = Some(models::Compression::Gzip);
        let result = filter_pipeline(&request_data, &bytes).unwrap();
        assert_eq!(data.as_ref(), result);
    }
}
