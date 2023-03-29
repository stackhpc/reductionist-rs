use crate::error::ActiveStorageError;
use crate::models;

use axum::body::Bytes;

/// Trait for array elements.
pub trait Element:
    Clone
    + Copy
    + PartialOrd
    + num_traits::FromPrimitive
    + num_traits::Zero
    + std::fmt::Debug
    + std::ops::Add<Output = Self>
    + std::ops::Div<Output = Self>
    + zerocopy::AsBytes
    + zerocopy::FromBytes
{
}

/// Blanket implementation of Element.
impl<T> Element for T where
    T: Clone
        + Copy
        + PartialOrd
        + num_traits::FromPrimitive
        + num_traits::One
        + num_traits::Zero
        + std::fmt::Debug
        + std::ops::Add<Output = Self>
        + std::ops::Div<Output = Self>
        + zerocopy::AsBytes
        + zerocopy::FromBytes
{
}

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
        request_data: &models::RequestData,
        data: &Bytes,
    ) -> Result<models::Response, ActiveStorageError>;
}

/// Trait for active storage operations on numerical data.
///
/// This trait provides an entry point into the type system based on the runtime `dtype` value.
pub trait NumOperation: Operation {
    fn execute_t<T: Element>(
        request_data: &models::RequestData,
        data: &Bytes,
    ) -> Result<models::Response, ActiveStorageError>;
}

impl<T: NumOperation> Operation for T {
    /// Execute the operation.
    ///
    /// This method dispatches to `execute_t` based on the `dtype`.
    fn execute(
        request_data: &models::RequestData,
        data: &Bytes,
    ) -> Result<models::Response, ActiveStorageError> {
        // Convert runtime data type into concrete types.
        match request_data.dtype {
            models::DType::Int32 => Self::execute_t::<i32>(request_data, data),
            models::DType::Int64 => Self::execute_t::<i64>(request_data, data),
            models::DType::Uint32 => Self::execute_t::<u32>(request_data, data),
            models::DType::Uint64 => Self::execute_t::<u64>(request_data, data),
            models::DType::Float32 => Self::execute_t::<f32>(request_data, data),
            models::DType::Float64 => Self::execute_t::<f64>(request_data, data),
        }
    }
}

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
