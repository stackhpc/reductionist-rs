//! Numerical operations.
//!
//! Each operation is implemented as a struct that implements the
//! [Operation](crate::operation::Operation) trait.

use crate::error::ActiveStorageError;
use crate::models;
use crate::operation::{ArrayOp, OperationResult};

use ndarray::ArrayView;
use ndarray_stats::{errors::MinMaxError, QuantileExt};

/// Return the number of selected elements in the array.
pub struct Count {}

impl<T> ArrayOp<Count> for T {
    type Res = u64;

    fn execute_array(
        _operation: &Count,
        _request_data: &models::RequestData,
        array: &ArrayView<T, ndarray::Dim<ndarray::IxDynImpl>>,
    ) -> Result<OperationResult<Self::Res>, ActiveStorageError> {
        let result = array
            .len()
            .try_into()
            .map_err(ActiveStorageError::TryFromInt)?;
        Ok(OperationResult::new(result, models::DType::Int64, vec![]))
    }
}

/// Return the maximum of selected elements in the array.
pub struct Max {}

impl<T> ArrayOp<Max> for T
where
    T: Copy + PartialOrd + zerocopy::AsBytes,
{
    type Res = T;

    fn execute_array(
        _operation: &Max,
        request_data: &models::RequestData,
        array: &ArrayView<T, ndarray::Dim<ndarray::IxDynImpl>>,
    ) -> Result<OperationResult<Self::Res>, ActiveStorageError> {
        let result = array
            .max()
            .map_err(|err| match err {
                MinMaxError::EmptyInput => ActiveStorageError::EmptyArray { operation: "max" },
                MinMaxError::UndefinedOrder => panic!("unexpected undefined order error for max"),
            })
            .copied()?;
        Ok(OperationResult::new(result, request_data.dtype, vec![]))
    }
}

/// Return the mean of selected elements in the array.
pub struct Mean {}

impl<T> ArrayOp<Mean> for T
where
    T: Copy
        + num_traits::FromPrimitive
        + num_traits::Zero
        + std::ops::Div<Output = T>
        + zerocopy::AsBytes,
{
    type Res = T;

    fn execute_array(
        _operation: &Mean,
        request_data: &models::RequestData,
        array: &ArrayView<T, ndarray::Dim<ndarray::IxDynImpl>>,
    ) -> Result<OperationResult<Self::Res>, ActiveStorageError> {
        let result = array
            .mean()
            .ok_or(ActiveStorageError::EmptyArray { operation: "mean" })?;
        Ok(OperationResult::new(result, request_data.dtype, vec![]))
    }
}

/// Return the minimum of selected elements in the array.
pub struct Min {}

impl<T> ArrayOp<Min> for T
where
    T: Copy + PartialOrd + zerocopy::AsBytes,
{
    type Res = T;

    fn execute_array(
        _operation: &Min,
        request_data: &models::RequestData,
        array: &ArrayView<T, ndarray::Dim<ndarray::IxDynImpl>>,
    ) -> Result<OperationResult<Self::Res>, ActiveStorageError> {
        let result = array
            .min()
            .map_err(|err| match err {
                MinMaxError::EmptyInput => ActiveStorageError::EmptyArray { operation: "min" },
                MinMaxError::UndefinedOrder => panic!("unexpected undefined order error for min"),
            })
            .copied()?;
        Ok(OperationResult::new(result, request_data.dtype, vec![]))
    }
}

/// Return all selected elements in the array.
//pub struct Select {}
//
//// FIXME: explicit lifetime name needed here
//impl<T> ArrayOp<Select> for T
//where
//    T: Copy
//        + zerocopy::AsBytes
//{
//    type Res = Vec<T>; // doesn't implement AsBytes. Need a wrapper type?
//
//    fn execute_array(
//        _operation: &Select,
//        request_data: &models::RequestData,
//        array: &ArrayView<T, ndarray::Dim<ndarray::IxDynImpl>>,
//    ) -> Result<OperationResult<Self::Res>, ActiveStorageError> {
//        // Transpose Fortran ordered arrays before iterating.
//        let result = if !array.is_standard_layout() {
//            array.t()
//        } else {
//            array
//        }.iter().copied().collect::<Vec<T>>();
//        let shape = array.shape().to_vec();
//        Ok(OperationResult::new(result, request_data.dtype, shape))
//    }
//}

/// Return the sum of selected elements in the array.
pub struct Sum {}

impl<T> ArrayOp<Sum> for T
where
    T: Copy + num_traits::Zero + zerocopy::AsBytes,
{
    type Res = T;

    fn execute_array(
        _operation: &Sum,
        request_data: &models::RequestData,
        array: &ArrayView<T, ndarray::Dim<ndarray::IxDynImpl>>,
    ) -> Result<OperationResult<Self::Res>, ActiveStorageError> {
        let result = array.sum();
        Ok(OperationResult::new(result, request_data.dtype, vec![]))
    }
}

// TODO: fix tests...

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
        let response = Count::execute(&request_data, &bytes).unwrap();
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
        let response = Max::execute(&request_data, &bytes).unwrap();
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
        let response = Mean::execute(&request_data, &bytes).unwrap();
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
        let response = Min::execute(&request_data, &bytes).unwrap();
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
        let response = Select::execute(&request_data, &bytes).unwrap();
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
        let response = Select::execute(&request_data, &bytes).unwrap();
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
        let response = Select::execute(&request_data, &bytes).unwrap();
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
        let response = Sum::execute(&request_data, &bytes).unwrap();
        let expected: u32 = 0x04030201 + 0x08070605;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Uint32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
    }
}
