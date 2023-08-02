use crate::models::*;
use crate::types::{ByteOrder, Missing};

use url::Url;

/// Create a RequestData object with only required fields set.
pub(crate) fn get_test_request_data() -> RequestData {
    RequestData {
        source: Url::parse("http://example.com").unwrap(),
        bucket: "bar".to_string(),
        object: "baz".to_string(),
        dtype: DType::Int32,
        byte_order: None,
        offset: None,
        size: None,
        shape: None,
        order: None,
        selection: None,
        compression: None,
        filters: None,
        missing: None,
    }
}

/// Create a RequestData object with all fields set.
pub(crate) fn get_test_request_data_optional() -> RequestData {
    RequestData {
        source: Url::parse("http://example.com").unwrap(),
        bucket: "bar".to_string(),
        object: "baz".to_string(),
        dtype: DType::Int32,
        byte_order: Some(ByteOrder::Little),
        offset: Some(4),
        size: Some(8),
        shape: Some(vec![2, 5]),
        order: Some(Order::C),
        selection: Some(vec![Slice::new(1, 2, 3), Slice::new(4, 5, 6)]),
        compression: Some(Compression::Gzip),
        filters: Some(vec![Filter::Shuffle { element_size: 4 }]),
        missing: Some(Missing::MissingValue(42.into())),
    }
}
