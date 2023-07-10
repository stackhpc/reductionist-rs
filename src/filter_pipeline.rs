//! Compression and filter pipeline.

use crate::compression;
use crate::error::ActiveStorageError;
use crate::filters;
use crate::models;

use axum::body::Bytes;

/// Returns data after applying a filter pipeline.
///
/// The pipeline is applied in the reverse order to when the data was written.
///
/// # Arguments
///
/// * `request_data`: RequestData object for the request
/// * `data`: Data [Bytes](axum::body::Bytes) to apply the pipeline to.
pub fn filter_pipeline(
    request_data: &models::RequestData,
    data: &Bytes,
) -> Result<Bytes, ActiveStorageError> {
    // Make a mutable shallow copy of the data.
    let mut data = data.clone();
    // First decompress.
    if let Some(compression) = request_data.compression {
        data = compression::decompress(compression, &data)?
    };
    // Then decode the filters in reverse order.
    if let Some(filters) = &request_data.filters {
        for filter in filters.iter().rev() {
            data = filters::decode(filter, &data)?
        }
    };
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use flate2::read::{GzEncoder, ZlibEncoder};
    use flate2::Compression;
    use std::io::Read;

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

    #[test]
    fn test_filter_pipeline_shuffle() {
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let bytes = Bytes::copy_from_slice(&data);
        let shuffled = filters::shuffle::test_utils::shuffle(&bytes, 4);
        let mut request_data = test_utils::get_test_request_data();
        request_data.filters = Some(vec![models::Filter::Shuffle { element_size: 4 }]);
        let result = filter_pipeline(&request_data, &shuffled).unwrap();
        assert_eq!(data.as_ref(), result);
    }

    #[test]
    fn test_filter_pipeline_shuffle_zlib() {
        let data: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        let bytes = Bytes::copy_from_slice(&data);
        let shuffled = filters::shuffle::test_utils::shuffle(&bytes, 4);
        let bytes = compress_zlib(shuffled.as_ref());
        let mut request_data = test_utils::get_test_request_data();
        request_data.compression = Some(models::Compression::Zlib);
        request_data.filters = Some(vec![models::Filter::Shuffle { element_size: 4 }]);
        let result = filter_pipeline(&request_data, &bytes).unwrap();
        assert_eq!(data.as_ref(), result.as_ref());
    }

    #[test]
    fn test_filter_pipeline_shuffle_x2_zlib() {
        // Test multiple filters.
        // Currently we only have shuffle, so run it twice with different element types.
        let data: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        let bytes = Bytes::copy_from_slice(&data);
        let shuffled = filters::shuffle::test_utils::shuffle(&bytes, 4);
        let reshuffled = filters::shuffle::test_utils::shuffle(&shuffled, 2);
        let bytes = compress_zlib(reshuffled.as_ref());
        let mut request_data = test_utils::get_test_request_data();
        request_data.compression = Some(models::Compression::Zlib);
        request_data.filters = Some(vec![
            models::Filter::Shuffle { element_size: 4 },
            models::Filter::Shuffle { element_size: 2 },
        ]);
        let result = filter_pipeline(&request_data, &bytes).unwrap();
        assert_eq!(data.as_ref(), result.as_ref());
    }
}
