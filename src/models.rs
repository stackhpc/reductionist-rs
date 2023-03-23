use axum::body::Bytes;
use serde::{Deserialize, Serialize};
use strum_macros::Display;
use url::Url;
use validator::{Validate, ValidationError};

#[derive(Clone, Copy, Debug, Deserialize, Display, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DType {
    Int32,
    Int64,
    Uint32,
    Uint64,
    Float32,
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

#[derive(Debug, Deserialize, PartialEq)]
pub enum Order {
    C,
    F,
}

// NOTE: In serde, structs can be deserialised from sequences or maps. This allows us to support
// the [<start>, <end>, <stride>] API, with the convenience of named fields.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
#[validate(schema(function = "validate_slice"))]
pub struct Slice {
    pub start: usize,
    pub end: usize,
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

#[derive(Debug, Deserialize, PartialEq, Validate)]
#[serde(deny_unknown_fields)]
#[validate(schema(function = "validate_request_data"))]
pub struct RequestData {
    // TODO: Investigate using lifetimes to enable zero-copy: https://serde.rs/lifetimes.html
    pub source: Url,
    #[validate(length(min = 1, message = "bucket must not be empty"))]
    pub bucket: String,
    #[validate(length(min = 1, message = "object must not be empty"))]
    pub object: String,
    pub dtype: DType,
    pub offset: Option<usize>,
    #[validate(range(min = 1, message = "size must be greater than 0"))]
    pub size: Option<usize>,
    #[validate(
        length(min = 1, message = "shape length must be greater than 0"),
        custom = "validate_shape"
    )]
    pub shape: Option<Vec<usize>>,
    pub order: Option<Order>,
    #[validate]
    #[validate(length(min = 1, message = "selection length must be greater than 0"))]
    pub selection: Option<Vec<Slice>>,
}

fn validate_shape(shape: &[usize]) -> Result<(), ValidationError> {
    if shape.iter().any(|index| *index == 0) {
        return Err(ValidationError::new("shape indices must be greater than 0"));
    }
    Ok(())
}

fn validate_slice(slice: &Slice) -> Result<(), ValidationError> {
    if slice.end <= slice.start {
        return Err(ValidationError::new(
            "Selection end must be greater than start",
        ));
    }
    Ok(())
}

fn validate_request_data(request_data: &RequestData) -> Result<(), ValidationError> {
    // Validation of multiple fields in RequestData.
    // TODO: More validation of shape & selection vs. size
    // TODO: More validation that selection fits in shape
    if let Some(size) = &request_data.size {
        if size % request_data.dtype.size_of() != 0 {
            return Err(ValidationError::new(
                "Size must be a multiple of dtype size in bytes",
            ));
        }
    };
    match (&request_data.shape, &request_data.selection) {
        (Some(shape), Some(selection)) => {
            if shape.len() != selection.len() {
                return Err(ValidationError::new(
                    "Shape and selection must have the same length",
                ));
            }
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
    pub body: Bytes,
    pub dtype: DType,
    pub shape: Vec<usize>,
}

impl Response {
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
            shape: Some(vec![1, 2]),
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
                Token::U32(1),
                Token::U32(2),
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
        let json = r#"{"source": "http://example.com", "bucket": "bar", "object": "baz", "dtype": "int32", "offset": 4, "size": 8, "shape": [1, 2], "order": "C", "selection": [[1, 2, 3], [4, 5, 6]]}"#;
        let request_data = serde_json::from_str::<RequestData>(json).unwrap();
        assert_eq!(request_data, get_test_request_data_optional());
    }
}
