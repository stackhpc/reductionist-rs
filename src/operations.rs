//! Numerical operations.
//!
//! Each operation is implemented as a struct that implements the
//! [Operation](crate::operation::Operation) trait.

use crate::array;
use crate::models;
use crate::operation::{Element, NumOperation};

use axum::body::Bytes;
use ndarray_stats::QuantileExt;
// Bring trait into scope to use as_bytes method.
use zerocopy::AsBytes;

/// Return the number of selected elements in the array.
pub struct Count {}

impl NumOperation for Count {
    fn execute_t<T: Element>(request_data: &models::RequestData, data: &Bytes) -> models::Response {
        let array = array::build_array::<T>(request_data, data);
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info.unwrap());
        // FIXME: endianness?
        // FIXME: handle errors
        let len = i64::try_from(sliced.len()).unwrap();
        let body = len.to_le_bytes();
        // Need to copy to provide ownership to caller.
        let body = Bytes::copy_from_slice(&body);
        models::Response::new(body, models::DType::Int64, vec![])
    }
}

/// Return the maximum of selected elements in the array.
pub struct Max {}

impl NumOperation for Max {
    fn execute_t<T: Element>(request_data: &models::RequestData, data: &Bytes) -> models::Response {
        let array = array::build_array::<T>(request_data, data);
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info.unwrap());
        // FIXME: endianness?
        // FIXME: handle errors
        let body = sliced.max().unwrap().as_bytes();
        // Need to copy to provide ownership to caller.
        let body = Bytes::copy_from_slice(body);
        models::Response::new(body, request_data.dtype, vec![])
    }
}

/// Return the mean of selected elements in the array.
pub struct Mean {}

impl NumOperation for Mean {
    fn execute_t<T: Element>(request_data: &models::RequestData, data: &Bytes) -> models::Response {
        let array = array::build_array::<T>(request_data, data);
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info.unwrap());
        // FIXME: endianness?
        // FIXME: handle errors
        let body = sliced.mean().unwrap();
        let body = body.as_bytes();
        // Need to copy to provide ownership to caller.
        let body = Bytes::copy_from_slice(body);
        models::Response::new(body, request_data.dtype, vec![])
    }
}

/// Return the minimum of selected elements in the array.
pub struct Min {}

impl NumOperation for Min {
    fn execute_t<T: Element>(request_data: &models::RequestData, data: &Bytes) -> models::Response {
        let array = array::build_array::<T>(request_data, data);
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info.unwrap());
        // FIXME: endianness?
        // FIXME: handle errors
        let body = sliced.min().unwrap().as_bytes();
        // Need to copy to provide ownership to caller.
        let body = Bytes::copy_from_slice(body);
        models::Response::new(body, request_data.dtype, vec![])
    }
}

/// Return all selected elements in the array.
pub struct Select {}

impl NumOperation for Select {
    fn execute_t<T: Element>(request_data: &models::RequestData, data: &Bytes) -> models::Response {
        let array = array::build_array::<T>(request_data, data);
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info.unwrap());
        let shape = sliced.shape().to_vec();
        // Transpose Fortran ordered arrays before iterating.
        let body = if !array.is_standard_layout() {
            let sliced_ordered = sliced.t();
            // FIXME: endianness?
            sliced_ordered.iter().copied().collect::<Vec<T>>()
        } else {
            // FIXME: endianness?
            sliced.iter().copied().collect::<Vec<T>>()
        };
        let body = body.as_bytes();
        // Need to copy to provide ownership to caller.
        let body = Bytes::copy_from_slice(body);
        models::Response::new(body, request_data.dtype, shape)
    }
}

/// Return the sum of selected elements in the array.
pub struct Sum {}

impl NumOperation for Sum {
    fn execute_t<T: Element>(request_data: &models::RequestData, data: &Bytes) -> models::Response {
        let array = array::build_array::<T>(request_data, data);
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info.unwrap());
        // FIXME: endianness?
        let body = sliced.sum();
        let body = body.as_bytes();
        // Need to copy to provide ownership to caller.
        let body = Bytes::copy_from_slice(body);
        models::Response::new(body, request_data.dtype, vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::operation::Operation;

    use url::Url;

    #[test]
    fn count_i32_1d() {
        let request_data = models::RequestData {
            source: Url::parse("http://example.com").unwrap(),
            bucket: "bar".to_string(),
            object: "baz".to_string(),
            dtype: models::DType::Int32,
            offset: None,
            size: None,
            shape: None,
            order: None,
            selection: None,
        };
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let bytes = Bytes::copy_from_slice(&data);
        let response = Count::execute(&request_data, &bytes);
        // Count is always i64.
        let expected: i64 = 2;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Int64, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
    }

    #[test]
    fn max_i64_1d() {
        let request_data = models::RequestData {
            source: Url::parse("http://example.com").unwrap(),
            bucket: "bar".to_string(),
            object: "baz".to_string(),
            dtype: models::DType::Int64,
            offset: None,
            size: None,
            shape: None,
            order: None,
            selection: None,
        };
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let bytes = Bytes::copy_from_slice(&data);
        let response = Max::execute(&request_data, &bytes);
        let expected: i64 = 0x0807060504030201;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Int64, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
    }

    #[test]
    fn mean_u32_1d() {
        let request_data = models::RequestData {
            source: Url::parse("http://example.com").unwrap(),
            bucket: "bar".to_string(),
            object: "baz".to_string(),
            dtype: models::DType::Uint32,
            offset: None,
            size: None,
            shape: None,
            order: None,
            selection: None,
        };
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let bytes = Bytes::copy_from_slice(&data);
        let response = Mean::execute(&request_data, &bytes);
        let expected: i32 = (0x08070605 + 0x04030201) / 2;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Uint32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
    }

    #[test]
    fn min_u64_1d() {
        let request_data = models::RequestData {
            source: Url::parse("http://example.com").unwrap(),
            bucket: "bar".to_string(),
            object: "baz".to_string(),
            dtype: models::DType::Uint64,
            offset: None,
            size: None,
            shape: None,
            order: None,
            selection: None,
        };
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let bytes = Bytes::copy_from_slice(&data);
        let response = Min::execute(&request_data, &bytes);
        let expected: u64 = 0x0807060504030201;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Uint64, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
    }

    #[test]
    fn select_f32_1d() {
        let request_data = models::RequestData {
            source: Url::parse("http://example.com").unwrap(),
            bucket: "bar".to_string(),
            object: "baz".to_string(),
            dtype: models::DType::Float32,
            offset: None,
            size: None,
            shape: None,
            order: None,
            selection: None,
        };
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let bytes = Bytes::copy_from_slice(&data);
        let response = Select::execute(&request_data, &bytes);
        let expected: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![2], response.shape);
    }

    #[test]
    fn select_f64_2d() {
        let request_data = models::RequestData {
            source: Url::parse("http://example.com").unwrap(),
            bucket: "bar".to_string(),
            object: "baz".to_string(),
            dtype: models::DType::Float64,
            offset: None,
            size: None,
            shape: Some(vec![2, 1]),
            order: None,
            selection: None,
        };
        let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let bytes = Bytes::copy_from_slice(&data);
        let response = Select::execute(&request_data, &bytes);
        let expected: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(16, response.body.len());
        assert_eq!(models::DType::Float64, response.dtype);
        assert_eq!(vec![2, 1], response.shape);
    }

    #[test]
    fn select_f32_2d_with_selection() {
        let request_data = models::RequestData {
            source: Url::parse("http://example.com").unwrap(),
            bucket: "bar".to_string(),
            object: "baz".to_string(),
            dtype: models::DType::Float32,
            offset: None,
            size: None,
            shape: Some(vec![2, 2]),
            order: None,
            selection: Some(vec![
                models::Slice::new(0, 2, 1),
                models::Slice::new(1, 2, 1),
            ]),
        };
        // 2x2 array, select second row of each column.
        // [[0x04030201, 0x08070605], [0x12111009, 0x16151413]]
        let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let bytes = Bytes::copy_from_slice(&data);
        let response = Select::execute(&request_data, &bytes);
        // [[0x08070605], [0x16151413]]
        let expected: [u8; 8] = [5, 6, 7, 8, 13, 14, 15, 16];
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![2, 1], response.shape);
    }

    #[test]
    fn sum_u32_1d() {
        let request_data = models::RequestData {
            source: Url::parse("http://example.com").unwrap(),
            bucket: "bar".to_string(),
            object: "baz".to_string(),
            dtype: models::DType::Uint32,
            offset: None,
            size: None,
            shape: None,
            order: None,
            selection: None,
        };
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let bytes = Bytes::copy_from_slice(&data);
        let response = Sum::execute(&request_data, &bytes);
        let expected: u32 = 0x04030201 + 0x08070605;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Uint32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
    }
}
