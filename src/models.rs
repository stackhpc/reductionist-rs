//! Data types and associated functions and methods

use axum::body::Bytes;
use serde::{Deserialize, Serialize};
use strum_macros::Display;
use url::Url;
use validator::{Validate, ValidationError};

/// Supported numerical data types
#[derive(Clone, Copy, Debug, Deserialize, Display, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DType {
    /// [i32]
    Int32,
    /// [i64]
    Int64,
    /// [u64]
    Uint32,
    /// [u64]
    Uint64,
    /// [f64]
    Float32,
    /// [f64]
    Float64,
}

impl DType {
    /// Returns the size of the associated type in bytes.
    pub fn size_of(self) -> usize {
        match self {
            Self::Int32 => std::mem::size_of::<i32>(),
            Self::Int64 => std::mem::size_of::<i64>(),
            Self::Uint32 => std::mem::size_of::<u32>(),
            Self::Uint64 => std::mem::size_of::<u64>(),
            Self::Float32 => std::mem::size_of::<f32>(),
            Self::Float64 => std::mem::size_of::<f64>(),
        }
    }
}

/// Array ordering
///
/// Defines an ordering for multi-dimensional arrays.
#[derive(Debug, Deserialize, PartialEq)]
pub enum Order {
    /// Row-major (C) ordering
    C,
    /// Column-major (Fortran) ordering
    F,
}

/// A slice of a single dimension of an array
///
/// The API uses NumPy slice semantics:
///
/// When start or end is negative:
/// * positive_start = start + length
/// * positive_end = end + length
/// Start and end are clamped:
/// * positive_start = min(positive_start, 0)
/// * positive_end + max(positive_end, length)
/// When the stride is positive:
/// * positive_start <= i < positive_end
/// When the stride is negative:
/// * positive_end <= i < positive_start
// NOTE: In serde, structs can be deserialised from sequences or maps. This allows us to support
// the [<start>, <end>, <stride>] API, with the convenience of named fields.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
#[validate(schema(function = "validate_slice"))]
pub struct Slice {
    /// Start of the slice
    pub start: isize,
    /// End of the slice
    pub end: isize,
    /// Stride size
    pub stride: isize,
}

impl Slice {
    /// Return a new Slice object.
    #[allow(dead_code)]
    pub fn new(start: isize, end: isize, stride: isize) -> Self {
        Slice { start, end, stride }
    }
}

/// Compression algorithm
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "id")]
pub enum Compression {
    /// Gzip
    Gzip,
    /// Zlib
    Zlib,
}

/// Request data for operations
#[derive(Debug, Deserialize, PartialEq, Validate)]
#[serde(deny_unknown_fields)]
#[validate(schema(function = "validate_request_data"))]
pub struct RequestData {
    /// URL of the S3-compatible object store
    // TODO: Investigate using lifetimes to enable zero-copy: https://serde.rs/lifetimes.html
    pub source: Url,
    /// S3 bucket containing the object
    #[validate(length(min = 1, message = "bucket must not be empty"))]
    pub bucket: String,
    /// S3 object containing the data
    #[validate(length(min = 1, message = "object must not be empty"))]
    pub object: String,
    /// Data type
    pub dtype: DType,
    /// Offset in bytes of the numerical data within the object
    pub offset: Option<usize>,
    /// Size in bytes of the numerical data from the offset
    #[validate(range(min = 1, message = "size must be greater than 0"))]
    pub size: Option<usize>,
    /// Shape of the multi-dimensional array
    #[validate(
        length(min = 1, message = "shape length must be greater than 0"),
        custom = "validate_shape"
    )]
    pub shape: Option<Vec<usize>>,
    /// Order of the multi-dimensional array
    pub order: Option<Order>,
    /// Subset of the data to operate on
    #[validate]
    #[validate(length(min = 1, message = "selection length must be greater than 0"))]
    pub selection: Option<Vec<Slice>>,
    /// Compression filter name
    pub compression: Option<Compression>,
}

/// Validate an array shape
fn validate_shape(shape: &[usize]) -> Result<(), ValidationError> {
    if shape.iter().any(|index| *index == 0) {
        return Err(ValidationError::new("shape indices must be greater than 0"));
    }
    Ok(())
}

/// Validate an array slice
fn validate_slice(slice: &Slice) -> Result<(), ValidationError> {
    if slice.stride == 0 {
        let mut error = ValidationError::new("Selection stride must not be equal to zero");
        error.add_param("stride".into(), &slice.stride);
        return Err(error);
    }
    Ok(())
}

/// Validate that a shape and selection are consistent
fn validate_shape_selection(
    shape: &Vec<usize>,
    selection: &Vec<Slice>,
) -> Result<(), ValidationError> {
    if shape.len() != selection.len() {
        let mut error = ValidationError::new("Shape and selection must have the same length");
        error.add_param("shape".into(), &shape.len());
        error.add_param("selection".into(), &selection.len());
        return Err(error);
    }
    Ok(())
}

/// Validate raw data size against data type and shape.
///
/// # Arguments
///
/// * `raw_size`: Raw (uncompressed) size of the data in bytes.
/// * `dtype`: Data type
/// * `shape`: Optional shape of the multi-dimensional array
pub fn validate_raw_size(
    raw_size: usize,
    dtype: DType,
    shape: &Option<Vec<usize>>,
) -> Result<(), ValidationError> {
    let dtype_size = dtype.size_of();
    if let Some(shape) = shape {
        let expected_size = shape.iter().product::<usize>() * dtype_size;
        if raw_size != expected_size {
            let mut error =
                ValidationError::new("Raw data size must be equal to the product of shape indices and dtype size in bytes");
            error.add_param("raw size".into(), &raw_size);
            error.add_param("dtype size".into(), &dtype_size);
            error.add_param("expected size".into(), &expected_size);
            return Err(error);
        }
    } else if raw_size % dtype_size != 0 {
        let mut error =
            ValidationError::new("Raw data size must be a multiple of dtype size in bytes");
        error.add_param("raw size".into(), &raw_size);
        error.add_param("dtype size".into(), &dtype_size);
        return Err(error);
    }
    Ok(())
}

/// Validate request data
fn validate_request_data(request_data: &RequestData) -> Result<(), ValidationError> {
    // Validation of multiple fields in RequestData.
    if let Some(size) = &request_data.size {
        // If the data is compressed then the size refers to the size of the compressed data, so we
        // can't validate it at this point.
        if request_data.compression.is_none() {
            validate_raw_size(*size, request_data.dtype, &request_data.shape)?;
        }
    };
    match (&request_data.shape, &request_data.selection) {
        (Some(shape), Some(selection)) => {
            validate_shape_selection(shape, selection)?;
        }
        (None, Some(_)) => {
            return Err(ValidationError::new(
                "Selection requires shape to be specified",
            ));
        }
        _ => (),
    };
    Ok(())
}

/// Response containing the result of a computation and associated metadata.
pub struct Response {
    /// Response data. May be a scalar or multi-dimensional array.
    pub body: Bytes,
    /// Data type of the response
    pub dtype: DType,
    /// Shape of the response
    pub shape: Vec<usize>,
    /// Number of non-missing elements operated on to generate response
    pub count: i64,
}

impl Response {
    /// Return a Response object
    pub fn new(body: Bytes, dtype: DType, shape: Vec<usize>, count: i64) -> Response {
        Response {
            body,
            dtype,
            shape,
            count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use serde_test::{assert_de_tokens, assert_de_tokens_error, Token};

    // The following tests use serde_test to validate the correct function of the deserialiser.
    // The validations are also tested.

    #[test]
    fn test_required_fields() {
        let request_data = test_utils::get_test_request_data();
        assert_de_tokens(
            &request_data,
            &[
                Token::Struct {
                    name: "RequestData",
                    len: 2,
                },
                Token::Str("source"),
                Token::Str("http://example.com"),
                Token::Str("bucket"),
                Token::Str("bar"),
                Token::Str("object"),
                Token::Str("baz"),
                Token::Str("dtype"),
                Token::Enum { name: "DType" },
                Token::Str("int32"),
                Token::Unit,
                Token::StructEnd,
            ],
        );
        request_data.validate().unwrap()
    }

    #[test]
    fn test_optional_fields() {
        let request_data = test_utils::get_test_request_data_optional();
        assert_de_tokens(
            &request_data,
            &[
                Token::Struct {
                    name: "RequestData",
                    len: 2,
                },
                Token::Str("source"),
                Token::Str("http://example.com"),
                Token::Str("bucket"),
                Token::Str("bar"),
                Token::Str("object"),
                Token::Str("baz"),
                Token::Str("dtype"),
                Token::Enum { name: "DType" },
                Token::Str("int32"),
                Token::Unit,
                Token::Str("offset"),
                Token::Some,
                Token::U32(4),
                Token::Str("size"),
                Token::Some,
                Token::U32(8),
                Token::Str("shape"),
                Token::Some,
                Token::Seq { len: Some(2) },
                Token::U32(2),
                Token::U32(5),
                Token::SeqEnd,
                Token::Str("order"),
                Token::Some,
                Token::Enum { name: "Order" },
                Token::Str("C"),
                Token::Unit,
                Token::Str("selection"),
                Token::Some,
                Token::Seq { len: Some(2) },
                Token::Seq { len: Some(3) },
                Token::U32(1),
                Token::U32(2),
                Token::U32(3),
                Token::SeqEnd,
                Token::Seq { len: Some(3) },
                Token::U32(4),
                Token::U32(5),
                Token::U32(6),
                Token::SeqEnd,
                Token::SeqEnd,
                Token::Str("compression"),
                Token::Some,
                Token::Map { len: None },
                Token::Str("id"),
                Token::Str("gzip"),
                Token::MapEnd,
                Token::StructEnd,
            ],
        );
        request_data.validate().unwrap()
    }

    #[test]
    fn test_missing_source() {
        assert_de_tokens_error::<RequestData>(
            &[
                Token::Struct {
                    name: "RequestData",
                    len: 2,
                },
                Token::StructEnd,
            ],
            "missing field `source`",
        )
    }

    #[test]
    fn test_invalid_source() {
        assert_de_tokens_error::<RequestData>(
            &[
                Token::Struct {
                    name: "RequestData",
                    len: 2,
                },
                Token::Str("source"),
                Token::Str("foo"),
                Token::StructEnd,
            ],
            "invalid value: string \"foo\", expected relative URL without a base",
        )
    }

    #[test]
    fn test_missing_bucket() {
        assert_de_tokens_error::<RequestData>(
            &[
                Token::Struct {
                    name: "RequestData",
                    len: 2,
                },
                Token::Str("source"),
                Token::Str("http://example.com"),
                Token::StructEnd,
            ],
            "missing field `bucket`",
        )
    }

    #[test]
    #[should_panic(expected = "bucket must not be empty")]
    fn test_invalid_bucket() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.bucket = "".to_string();
        request_data.validate().unwrap()
    }

    #[test]
    fn test_missing_object() {
        assert_de_tokens_error::<RequestData>(
            &[
                Token::Struct {
                    name: "RequestData",
                    len: 2,
                },
                Token::Str("source"),
                Token::Str("http://example.com"),
                Token::Str("bucket"),
                Token::Str("bar"),
                Token::StructEnd,
            ],
            "missing field `object`",
        )
    }

    #[test]
    #[should_panic(expected = "object must not be empty")]
    fn test_invalid_object() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.object = "".to_string();
        request_data.validate().unwrap()
    }

    #[test]
    fn test_missing_dtype() {
        assert_de_tokens_error::<RequestData>(
            &[
                Token::Struct {
                    name: "RequestData",
                    len: 2,
                },
                Token::Str("source"),
                Token::Str("http://example.com"),
                Token::Str("bucket"),
                Token::Str("bar"),
                Token::Str("object"),
                Token::Str("baz"),
                Token::StructEnd,
            ],
            "missing field `dtype`",
        )
    }

    #[test]
    fn test_invalid_dtype() {
        assert_de_tokens_error::<RequestData>(&[
            Token::Struct { name: "RequestData", len: 2 },
            Token::Str("dtype"),
            Token::Enum { name: "DType" },
            Token::Str("foo"),
            Token::StructEnd
            ],
            "unknown variant `foo`, expected one of `int32`, `int64`, `uint32`, `uint64`, `float32`, `float64`"
        )
    }

    #[test]
    #[should_panic(expected = "size must be greater than 0")]
    fn test_invalid_size() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.size = Some(0);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "shape length must be greater than 0")]
    fn test_invalid_shape() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.shape = Some(vec![]);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "shape indices must be greater than 0")]
    fn test_invalid_shape_indices() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.shape = Some(vec![0]);
        request_data.validate().unwrap()
    }

    #[test]
    fn test_invalid_order() {
        assert_de_tokens_error::<RequestData>(
            &[
                Token::Struct {
                    name: "RequestData",
                    len: 2,
                },
                Token::Str("order"),
                Token::Some,
                Token::Enum { name: "Order" },
                Token::Str("foo"),
                Token::StructEnd,
            ],
            "unknown variant `foo`, expected `C` or `F`",
        )
    }

    #[test]
    #[should_panic(expected = "selection length must be greater than 0")]
    fn test_invalid_selection() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.selection = Some(vec![]);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "Selection stride must not be equal to zero")]
    fn test_invalid_selection2() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.selection = Some(vec![Slice::new(1, 2, 0)]);
        request_data.validate().unwrap()
    }

    #[test]
    fn test_selection_end_lt_start() {
        // Numpy sementics: start >= end yields an empty array
        let mut request_data = test_utils::get_test_request_data();
        request_data.shape = Some(vec![1]);
        request_data.selection = Some(vec![Slice::new(1, 0, 1)]);
        request_data.validate().unwrap()
    }

    #[test]
    fn test_selection_negative_stride() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.shape = Some(vec![1]);
        request_data.selection = Some(vec![Slice::new(1, 0, -1)]);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "Raw data size must be a multiple of dtype size in bytes")]
    fn test_invalid_size_for_dtype() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.size = Some(1);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(
        expected = "Raw data size must be equal to the product of shape indices and dtype size in bytes"
    )]
    fn test_invalid_size_for_shape() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.size = Some(4);
        request_data.shape = Some(vec![1, 2]);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "Shape and selection must have the same length")]
    fn test_shape_selection_mismatch() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.shape = Some(vec![1, 2]);
        request_data.selection = Some(vec![Slice::new(1, 2, 1)]);
        request_data.validate().unwrap()
    }

    #[test]
    fn test_selection_start_gt_shape() {
        // Numpy sementics: start > length yields an empty array
        let mut request_data = test_utils::get_test_request_data();
        request_data.shape = Some(vec![4]);
        request_data.selection = Some(vec![Slice::new(5, 5, 1)]);
        request_data.validate().unwrap()
    }

    #[test]
    fn test_selection_start_lt_negative_shape() {
        // Numpy sementics: start < -length gets clamped to zero
        let mut request_data = test_utils::get_test_request_data();
        request_data.shape = Some(vec![4]);
        request_data.selection = Some(vec![Slice::new(-5, 5, 1)]);
        request_data.validate().unwrap()
    }

    #[test]
    fn test_selection_end_gt_shape() {
        // Numpy semantics: end > length gets clamped to length
        let mut request_data = test_utils::get_test_request_data();
        request_data.shape = Some(vec![4]);
        request_data.selection = Some(vec![Slice::new(1, 5, 1)]);
        request_data.validate().unwrap()
    }

    #[test]
    fn test_selection_end_lt_negative_shape() {
        // Numpy semantics: end < -length gets clamped to zero
        let mut request_data = test_utils::get_test_request_data();
        request_data.shape = Some(vec![4]);
        request_data.selection = Some(vec![Slice::new(1, -5, 1)]);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "Selection requires shape to be specified")]
    fn test_selection_without_shape() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.selection = Some(vec![Slice::new(1, 2, 1)]);
        request_data.validate().unwrap()
    }

    #[test]
    fn test_invalid_compression() {
        assert_de_tokens_error::<RequestData>(
            &[
                Token::Struct {
                    name: "RequestData",
                    len: 2,
                },
                Token::Str("compression"),
                Token::Some,
                Token::Map { len: None },
                Token::Str("id"),
                Token::Str("foo"),
                Token::MapEnd,
            ],
            "unknown variant `foo`, expected `gzip` or `zlib`",
        )
    }

    #[test]
    fn test_unknown_field() {
        assert_de_tokens_error::<RequestData>(&[
            Token::Struct { name: "RequestData", len: 2 },
            Token::Str("foo"),
            Token::StructEnd
            ],
            "unknown field `foo`, expected one of `source`, `bucket`, `object`, `dtype`, `offset`, `size`, `shape`, `order`, `selection`, `compression`"
        )
    }

    // The following tests use JSON data, to check that the fields map as expected.

    #[test]
    fn test_json_required_fields() {
        let json = r#"{"source": "http://example.com", "bucket": "bar", "object": "baz", "dtype": "int32"}"#;
        let request_data = serde_json::from_str::<RequestData>(json).unwrap();
        assert_eq!(request_data, test_utils::get_test_request_data());
    }

    #[test]
    fn test_json_optional_fields() {
        let json = r#"{"source": "http://example.com", "bucket": "bar", "object": "baz", "dtype": "int32", "offset": 4, "size": 8, "shape": [2, 5], "order": "C", "selection": [[1, 2, 3], [4, 5, 6]], "compression": {"id": "gzip"}}"#;
        let request_data = serde_json::from_str::<RequestData>(json).unwrap();
        assert_eq!(request_data, test_utils::get_test_request_data_optional());
    }
}
