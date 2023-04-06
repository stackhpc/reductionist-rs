//! Interface for Active Storage operations

use crate::array;
use crate::error::ActiveStorageError;
use crate::models;

use axum::body::Bytes;
use ndarray::ArrayView;

/// Trait for active storage operations.
///
/// This forms the contract between the API layer and operations.
pub trait Operation {
    /// Execute the operation.
    ///
    /// Returns a [models::Response](crate::models::Response) object with response data.
    ///
    /// # Arguments
    ///
    /// * `request_data`: RequestData object for the request
    /// * `data`: Bytes containing data to operate on.
    fn execute(
        &self,
        request_data: &models::RequestData,
        data: &Bytes,
    ) -> Result<models::Response, ActiveStorageError>;
}

impl<T> Operation for T
where
    i32: NumOp<T>,
    i64: NumOp<T>,
    u32: NumOp<T>,
    u64: NumOp<T>,
    f32: NumOp<T>,
    f64: NumOp<T>,
{
    fn execute(
        &self,
        request_data: &models::RequestData,
        data: &Bytes,
    ) -> Result<models::Response, ActiveStorageError> {
        match request_data.dtype {
            models::DType::Int32 => i32::execute(self, request_data, data),
            models::DType::Int64 => i64::execute(self, request_data, data),
            models::DType::Uint32 => u32::execute(self, request_data, data),
            models::DType::Uint64 => u64::execute(self, request_data, data),
            models::DType::Float32 => f32::execute(self, request_data, data),
            models::DType::Float64 => f64::execute(self, request_data, data),
        }
    }
}

/// Trait for active storage operations on numerical data.
///
/// This trait provides an entry point into the type system based on the runtime `dtype` value.
pub trait NumOp<O> {
    fn execute(
        operation: &O,
        request_data: &models::RequestData,
        data: &Bytes,
    ) -> Result<models::Response, ActiveStorageError>
    where
        Self: Sized;
}

/// A typed operation result.
pub struct OperationResult<R: zerocopy::AsBytes> {
    /// Result data
    pub result: R,
    /// Data type of the result
    pub dtype: models::DType,
    /// Shape of the result
    pub shape: Vec<usize>,
}

impl<R: zerocopy::AsBytes> OperationResult<R> {
    pub fn new(result: R, dtype: models::DType, shape: Vec<usize>) -> Self {
        OperationResult {
            result,
            dtype,
            shape,
        }
    }
}

impl<R: zerocopy::AsBytes> From<OperationResult<R>> for models::Response {
    fn from(result: OperationResult<R>) -> models::Response {
        // FIXME: endianness
        let body = result.result.as_bytes();
        // Need to copy to provide ownership to caller.
        let body = Bytes::copy_from_slice(body);
        models::Response::new(body, result.dtype, result.shape)
    }
}

pub trait ArrayOp<O> {
    type Res: zerocopy::AsBytes;

    fn execute_array(
        operation: &O,
        request_data: &models::RequestData,
        array: &ArrayView<Self, ndarray::Dim<ndarray::IxDynImpl>>,
    ) -> Result<OperationResult<Self::Res>, ActiveStorageError>
    where
        Self: Sized;
}

impl<T, O> NumOp<O> for T
where
    T: zerocopy::AsBytes + zerocopy::FromBytes + ArrayOp<O>,
{
    fn execute(
        operation: &O,
        request_data: &models::RequestData,
        data: &Bytes,
    ) -> Result<models::Response, ActiveStorageError>
    where
        Self: Sized,
    {
        let array = array::build_array(request_data, data)?;
        let result = if let Some(selection) = &request_data.selection {
            let slice_info = array::build_slice_info::<T>(selection);
            let sliced = array.slice(slice_info);
            Self::execute_array(operation, request_data, &sliced)?
        } else {
            Self::execute_array(operation, request_data, &array)?
        };
        Ok(result.into())
    }
}

// TODO: fix tests

#[cfg(test)]
mod tests {
    use super::*;

    use url::Url;

    struct TestOp {}

    impl Operation for TestOp {
        fn execute(
            request_data: &models::RequestData,
            data: &Bytes,
        ) -> Result<models::Response, ActiveStorageError> {
            // Clone request body into response body.
            Ok(models::Response::new(
                data.clone(),
                request_data.dtype,
                vec![3],
            ))
        }
    }

    #[test]
    fn operation_u32() {
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
        let data = [1, 2, 3, 4];
        let bytes = Bytes::copy_from_slice(&data);
        let response = TestOp::execute(&request_data, &bytes).unwrap();
        assert_eq!(&[1, 2, 3, 4][..], response.body);
        assert_eq!(models::DType::Uint32, response.dtype);
        assert_eq!(vec![3], response.shape);
    }

    struct TestNumOp {}

    impl NumOperation for TestNumOp {
        fn execute_t<T: Element>(
            request_data: &models::RequestData,
            _data: &Bytes,
        ) -> Result<models::Response, ActiveStorageError> {
            // Write the name of the type parameter to the body.
            let body = std::any::type_name::<T>();
            Ok(models::Response::new(
                body.into(),
                request_data.dtype,
                vec![1, 2],
            ))
        }
    }

    #[test]
    fn num_operation_i64() {
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
        let data = [1, 2, 3, 4];
        let bytes = Bytes::copy_from_slice(&data);
        let response = TestNumOp::execute(&request_data, &bytes).unwrap();
        assert_eq!("i64", response.body);
        assert_eq!(models::DType::Int64, response.dtype);
        assert_eq!(vec![1, 2], response.shape);
    }
}
