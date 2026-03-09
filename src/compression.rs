//! (De)compression support.

use crate::error::ActiveStorageError;
use crate::models;

use axum::body::Bytes;
use blusc::{blosc2_cbuffer_sizes, blosc2_decompress};
use flate2::read::GzDecoder;
use std::io::Read;
use zune_inflate::{DeflateDecoder, DeflateOptions};

/// Decompresses some Bytes and returns the uncompressed data.
///
/// # Arguments
///
/// * `compression`: Compression algorithm
/// * `data`: Compressed data [Bytes]
pub fn decompress(
    compression: models::Compression,
    data: &Bytes,
) -> Result<Bytes, ActiveStorageError> {
    match compression {
        models::Compression::Blosc2 => decompress_blusc_blosc2(data),
        models::Compression::Gzip => decompress_flate2_gzip(data),
        models::Compression::Zlib => decompress_zune_zlib(data),
    }
}

fn decompress_blusc_blosc2(data: &Bytes) -> Result<Bytes, ActiveStorageError> {
    // We should be able to read the uncompressed size from the header of the compressed data,
    // and use that to allocate the output buffer.
    // If the uncompressed size is 0, then the compressed data is invalid.
    let (nbytes, _cbytes, _block_size) = blosc2_cbuffer_sizes(data);
    if nbytes == 0 {
        return Err(ActiveStorageError::DecompressionBlosc2 {
            error: "invalid blosc2 compressed data",
        });
    }
    // Create an 8-byte aligned Vec<u8> large enough to hold the uncompressed data.
    let mut output = maligned::align_first::<u8, maligned::A8>(nbytes);
    // Ensure the length matches the expected uncompressed size so the whole buffer is writable.
    output.resize(nbytes, 0u8);
    // The blosc2_decompress function will write the uncompressed data into the output buffer
    // and return the number of bytes written.
    // If the return value is 0, then the decompression failed.
    let written = blosc2_decompress(data, &mut output);
    if written <= 0 {
        return Err(ActiveStorageError::DecompressionBlosc2 {
            error: "failed to decompress blosc2 data",
        });
    }
    if written as usize != nbytes {
        return Err(ActiveStorageError::DecompressionBlosc2 {
            error: "unexpected decompressed size for blosc2 data",
        });
    }
    Ok(output.into())
}

fn decompress_flate2_gzip(data: &Bytes) -> Result<Bytes, ActiveStorageError> {
    let mut decoder = GzDecoder::<&[u8]>::new(data);
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

fn decompress_zune_zlib(data: &Bytes) -> Result<Bytes, ActiveStorageError> {
    let options = DeflateOptions::default().set_size_hint(data.len());
    let mut decoder = DeflateDecoder::new_with_options(data, options);
    let data = decoder.decode_zlib()?;
    Ok(data.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use blusc::{BLOSC_SHUFFLE, BLOSC2_MAX_OVERHEAD, blosc2_compress};
    use flate2::Compression;
    use flate2::read::{GzEncoder, ZlibEncoder};
    use zune_inflate::errors::DecodeErrorStatus;

    fn compress_blosc() -> Vec<u8> {
        // Adapted from blosc documentation.
        let input = b"hello world";
        // Allocate a buffer for the compressed data.
        // As the compressed data is block aligned the compressed size may be larger than the input size,
        // we need to allocate a buffer that is large enough to hold the compressed data.
        let mut compressed = vec![0u8; input.len() + BLOSC2_MAX_OVERHEAD];
        let cbytes = blosc2_compress(5, BLOSC_SHUFFLE as i32, 1, input, &mut compressed);
        // Ensure compression succeeded and the returned size is valid before slicing.
        assert!(cbytes > 0, "blosc2_compress failed, returned {}", cbytes);
        assert!(
            cbytes as usize <= compressed.len(),
            "blosc2_compress returned size {cbytes} larger than destination buffer {}",
            compressed.len()
        );
        compressed.truncate(cbytes as usize);
        compressed
    }

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
    fn test_decompress_blosc() {
        let compressed = compress_blosc();
        let result = decompress(models::Compression::Blosc2, &compressed.into()).unwrap();
        assert_eq!(result, b"hello world".as_ref());
        assert_eq!(result.as_ptr().align_offset(8), 0);
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
    fn test_decompress_invalid_blosc() {
        let invalid = b"invalid format";
        let err = decompress(models::Compression::Blosc2, &invalid.as_ref().into()).unwrap_err();
        match err {
            ActiveStorageError::DecompressionBlosc2 { error } => {
                assert_eq!(error, "invalid blosc2 compressed data");
            }
            err => panic!("unexpected error {err}"),
        }
    }

    #[test]
    fn test_decompress_invalid_gzip() {
        let invalid = b"invalid format";
        let err = decompress(models::Compression::Gzip, &invalid.as_ref().into()).unwrap_err();
        match err {
            ActiveStorageError::DecompressionFlate2(io_err) => {
                assert_eq!(io_err.kind(), std::io::ErrorKind::InvalidInput);
                assert_eq!(io_err.to_string(), "invalid gzip header");
            }
            err => panic!("unexpected error {err}"),
        }
    }

    #[test]
    fn test_decompress_invalid_zlib() {
        let invalid = b"invalid format";
        let err = decompress(models::Compression::Zlib, &invalid.as_ref().into()).unwrap_err();
        match err {
            ActiveStorageError::DecompressionZune(zune_err) => match zune_err.error {
                DecodeErrorStatus::GenericStr(message) => {
                    assert_eq!(message, "Unknown zlib compression method 9");
                }
                err => panic!("unexpected zune error {err:?}"),
            },
            err => panic!("unexpected error {err}"),
        }
    }
}
