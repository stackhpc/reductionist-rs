//! (De)compression support.

use crate::error::ActiveStorageError;
use crate::models;

use axum::body::Bytes;
use flate2::read::{GzDecoder, ZlibDecoder};
use std::io::Read;

/// Decompresses some Bytes and returns the uncompressed data.
///
/// # Arguments
///
/// * `compression`: Compression algorithm
/// * `data`: Compressed data [Bytes](axum::body::Bytes)
pub fn decompress(
    compression: models::Compression,
    data: &Bytes,
) -> Result<Bytes, ActiveStorageError> {
    let mut decoder: Box<dyn Read> = match compression {
        models::Compression::Gzip => Box::new(GzDecoder::<&[u8]>::new(data)),
        models::Compression::Zlib => Box::new(ZlibDecoder::<&[u8]>::new(data)),
    };
    // The data returned by the S3 client does not have any alignment guarantees. In order to
    // reinterpret the data as an array of numbers with a higher alignment than 1, we need to
    // return the data in Bytes object in which the underlying data has a higher alignment.
    // For now we're hard-coding an alignment of 8 bytes, although this should depend on the
    // data type, and potentially whether there are any SIMD requirements.
    // Create an 8-byte aligned Vec<u8>.
    // FIXME: The compressed length will not be enough to store the uncompressed data, and may
    // result in a change in the underlying buffer to one that is not correctly aligned.
    let mut buf = maligned::align_first::<u8, maligned::A8>(data.len());
    decoder.read_to_end(&mut buf)?;
    // Release any unnecessary capacity.
    buf.shrink_to(0);
    Ok(buf.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::{GzEncoder, ZlibEncoder};
    use flate2::Compression;

    fn compress_gzip() -> Vec<u8> {
        // Adapated from flate2 documentation.
        let mut result = Vec::<u8>::new();
        let input = b"hello world";
        let mut deflater = GzEncoder::new(&input[..], Compression::fast());
        deflater.read_to_end(&mut result).unwrap();
        result
    }

    fn compress_zlib() -> Vec<u8> {
        // Adapated from flate2 documentation.
        let mut result = Vec::<u8>::new();
        let input = b"hello world";
        let mut deflater = ZlibEncoder::new(&input[..], Compression::fast());
        deflater.read_to_end(&mut result).unwrap();
        result
    }

    #[test]
    fn test_decompress_gzip() {
        let compressed = compress_gzip();
        let result = decompress(models::Compression::Gzip, &compressed.into()).unwrap();
        assert_eq!(result, b"hello world".as_ref());
        assert_eq!(result.as_ptr().align_offset(8), 0);
    }

    #[test]
    fn test_decompress_zlib() {
        let compressed = compress_zlib();
        let result = decompress(models::Compression::Zlib, &compressed.into()).unwrap();
        assert_eq!(result, b"hello world".as_ref());
        assert_eq!(result.as_ptr().align_offset(8), 0);
    }

    #[test]
    fn test_decompress_invalid_gzip() {
        let invalid = b"invalid format";
        let err = decompress(models::Compression::Gzip, &invalid.as_ref().into()).unwrap_err();
        match err {
            ActiveStorageError::Decompression(io_err) => {
                assert_eq!(io_err.kind(), std::io::ErrorKind::InvalidInput);
                assert_eq!(io_err.to_string(), "invalid gzip header");
            }
            err => panic!("unexpected error {}", err),
        }
    }

    #[test]
    fn test_decompress_invalid_zlib() {
        let invalid = b"invalid format";
        let err = decompress(models::Compression::Zlib, &invalid.as_ref().into()).unwrap_err();
        match err {
            ActiveStorageError::Decompression(io_err) => {
                assert_eq!(io_err.kind(), std::io::ErrorKind::InvalidInput);
                assert_eq!(io_err.to_string(), "corrupt deflate stream");
            }
            err => panic!("unexpected error {}", err),
        }
    }
}
