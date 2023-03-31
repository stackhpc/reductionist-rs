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
    fn size_of(self) -> usize {
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
// NOTE: In serde, structs can be deserialised from sequences or maps. This allows us to support
// the [<start>, <end>, <stride>] API, with the convenience of named fields.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
#[validate(schema(function = "validate_slice"))]
pub struct Slice {
    /// Start of the slice
    pub start: usize,
    /// End of the slice
    pub end: usize,
    /// Stride size
    #[validate(range(min = 1, message = "stride must be greater than 0"))]
    pub stride: usize,
}

impl Slice {
    /// Return a new Slice object.
    #[allow(dead_code)]
    pub fn new(start: usize, end: usize, stride: usize) -> Self {
        Slice { start, end, stride }
    }
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
    if slice.end <= slice.start {
        let mut error = ValidationError::new("Selection end must be greater than start");
        error.add_param("start".into(), &slice.start);
        error.add_param("end".into(), &slice.end);
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
    for (shape_i, selection_i) in std::iter::zip(shape, selection) {
        if selection_i.end > *shape_i {
            let mut error = ValidationError::new(
                "Selection end must be less than or equal to corresponding shape index",
            );
            error.add_param("shape".into(), &shape_i);
            error.add_param("selection".into(), &selection_i);
            return Err(error);
        }
    }
    Ok(())
}

/// Validate request data
fn validate_request_data(request_data: &RequestData) -> Result<(), ValidationError> {
    // Validation of multiple fields in RequestData.
    // TODO: More validation of shape & selection vs. size
    if let Some(size) = &request_data.size {
        let dtype_size = request_data.dtype.size_of();
        if size % dtype_size != 0 {
            let mut error = ValidationError::new("Size must be a multiple of dtype size in bytes");
            error.add_param("size".into(), &size);
            error.add_param("dtype size".into(), &dtype_size);
            return Err(error);
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
}

impl Response {
    /// Return a Response object
    pub fn new(body: Bytes, dtype: DType, shape: Vec<usize>) -> Response {
        Response { body, dtype, shape }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_test::{assert_de_tokens, assert_de_tokens_error, Token};

    fn get_test_request_data() -> RequestData {
        RequestData {
            source: Url::parse("http://example.com").unwrap(),
            bucket: "bar".to_string(),
            object: "baz".to_string(),
            dtype: DType::Int32,
            offset: None,
            size: None,
            shape: None,
            order: None,
            selection: None,
        }
    }

    fn get_test_request_data_optional() -> RequestData {
        RequestData {
            source: Url::parse("http://example.com").unwrap(),
            bucket: "bar".to_string(),
            object: "baz".to_string(),
            dtype: DType::Int32,
            offset: Some(4),
            size: Some(8),
            shape: Some(vec![2, 5]),
            order: Some(Order::C),
            selection: Some(vec![Slice::new(1, 2, 3), Slice::new(4, 5, 6)]),
        }
    }

    // The following tests use serde_test to validate the correct function of the deserialiser.
    // The validations are also tested.

    #[test]
    fn test_required_fields() {
        let request_data = get_test_request_data();
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
        let request_data = get_test_request_data_optional();
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
        let mut request_data = get_test_request_data();
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
        let mut request_data = get_test_request_data();
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
        let mut request_data = get_test_request_data();
        request_data.size = Some(0);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "shape length must be greater than 0")]
    fn test_invalid_shape() {
        let mut request_data = get_test_request_data();
        request_data.shape = Some(vec![]);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "shape indices must be greater than 0")]
    fn test_invalid_shape_indices() {
        let mut request_data = get_test_request_data();
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
        let mut request_data = get_test_request_data();
        request_data.selection = Some(vec![]);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "stride must be greater than 0")]
    fn test_invalid_selection2() {
        let mut request_data = get_test_request_data();
        request_data.selection = Some(vec![Slice::new(1, 2, 0)]);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "Selection end must be greater than start")]
    fn test_invalid_selection3() {
        let mut request_data = get_test_request_data();
        request_data.selection = Some(vec![Slice::new(1, 1, 1)]);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "Size must be a multiple of dtype size in bytes")]
    fn test_invalid_size_for_dtype() {
        let mut request_data = get_test_request_data();
        request_data.size = Some(1);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "Shape and selection must have the same length")]
    fn test_shape_selection_mismatch() {
        let mut request_data = get_test_request_data();
        request_data.shape = Some(vec![1, 2]);
        request_data.selection = Some(vec![Slice::new(1, 2, 1)]);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(
        expected = "Selection end must be less than or equal to corresponding shape index"
    )]
    fn test_selection_end_gt_shape() {
        let mut request_data = get_test_request_data();
        request_data.shape = Some(vec![4]);
        request_data.selection = Some(vec![Slice::new(1, 5, 1)]);
        request_data.validate().unwrap()
    }

    #[test]
    #[should_panic(expected = "Selection requires shape to be specified")]
    fn test_selection_without_shape() {
        let mut request_data = get_test_request_data();
        request_data.selection = Some(vec![Slice::new(1, 2, 1)]);
        request_data.validate().unwrap()
    }
    #[test]
    fn test_unknown_field() {
        assert_de_tokens_error::<RequestData>(&[
            Token::Struct { name: "RequestData", len: 2 },
            Token::Str("foo"),
            Token::StructEnd
            ],
            "unknown field `foo`, expected one of `source`, `bucket`, `object`, `dtype`, `offset`, `size`, `shape`, `order`, `selection`"
        )
    }

    // The following tests use JSON data, to check that the fields map as expected.

    #[test]
    fn test_json_required_fields() {
        let json = r#"{"source": "http://example.com", "bucket": "bar", "object": "baz", "dtype": "int32"}"#;
        let request_data = serde_json::from_str::<RequestData>(json).unwrap();
        assert_eq!(request_data, get_test_request_data());
    }

    #[test]
    fn test_json_optional_fields() {
        let json = r#"{"source": "http://example.com", "bucket": "bar", "object": "baz", "dtype": "int32", "offset": 4, "size": 8, "shape": [2, 5], "order": "C", "selection": [[1, 2, 3], [4, 5, 6]]}"#;
        let request_data = serde_json::from_str::<RequestData>(json).unwrap();
        assert_eq!(request_data, get_test_request_data_optional());
    }
}
