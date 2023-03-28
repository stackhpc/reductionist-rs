//! Error handling.

use aws_sdk_s3::error::{GetObjectError, GetObjectErrorKind};
use aws_sdk_s3::types::{DisplayErrorContext, SdkError};
use aws_smithy_http::byte_stream::error::Error as ByteStreamError;
use axum::{
    http::header,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use ndarray::ShapeError;
use ndarray_stats::errors::MinMaxError;
use thiserror::Error;

/// Active Storage server error type
///
/// This type encapsulates the various errors that may occur.
/// Each variant may result in a different API error response.
#[derive(Debug, Error)]
pub enum ActiveStorageError {
    /// Attempt to perform an invalid operation on an empty array or selection
    #[error("cannot perform {operation} on empty array or selection")]
    EmptyArray { operation: &'static str },

    /// Error converting from bytes to a type
    #[error("failed to convert from bytes to {type_name}")]
    FromBytes { type_name: &'static str },

    /// Error performing minimum or maxiumum on an array
    #[error(transparent)]
    MinMax(#[from] MinMaxError),

    /// Error reading object data from S3
    #[error(transparent)]
    S3ByteStream(#[from] ByteStreamError),

    /// Error while retrieving an object from S3
    #[error(transparent)]
    S3GetObject(#[from] SdkError<GetObjectError>),

    /// Invalid array shape
    #[error(transparent)]
    ShapeInvalid(#[from] ShapeError),

    /// Error converting between integer types
    #[error(transparent)]
    TryFromInt(#[from] std::num::TryFromIntError),
}

// Tell axum how to convert `ActiveStorageError` into a response.
impl IntoResponse for ActiveStorageError {
    fn into_response(self) -> Response {
        let headers = [(&header::CONTENT_TYPE, mime::TEXT_PLAIN.to_string())];
        let message = self.to_string();
        match self {
            // Bad request
            ActiveStorageError::EmptyArray { operation: _ }
            | ActiveStorageError::MinMax(_)
            | ActiveStorageError::ShapeInvalid(_) => {
                (StatusCode::BAD_REQUEST, headers, message).into_response()
            }

            // Internal server error
            ActiveStorageError::FromBytes { type_name: _ }
            | ActiveStorageError::TryFromInt(_)
            | ActiveStorageError::S3ByteStream(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, headers, message).into_response()
            }

            ActiveStorageError::S3GetObject(sdk_error) => {
                let message = DisplayErrorContext(&sdk_error).to_string();
                match sdk_error {
                    // Internal server error
                    SdkError::ConstructionFailure(_)
                    | SdkError::DispatchFailure(_)
                    | SdkError::ResponseError(_)
                    | SdkError::TimeoutError(_) => {
                        (StatusCode::INTERNAL_SERVER_ERROR, headers, message).into_response()
                    }

                    SdkError::ServiceError(get_obj_error) => {
                        let message = if let Some(get_obj_message) = get_obj_error.err().message() {
                            get_obj_message.to_string()
                        } else {
                            message
                        };
                        match get_obj_error.into_err().kind {
                            GetObjectErrorKind::InvalidObjectState(_)
                            | GetObjectErrorKind::NoSuchKey(_) => {
                                (StatusCode::BAD_REQUEST, headers, message).into_response()
                            }

                            // FIXME: Quite a lot of error cases end up here - invalid username,
                            // password, etc.
                            GetObjectErrorKind::Unhandled(_) => {
                                (StatusCode::INTERNAL_SERVER_ERROR, headers, message)
                                    .into_response()
                            }

                            // The enum is marked as non-exhaustive
                            _ => (StatusCode::INTERNAL_SERVER_ERROR, headers, message)
                                .into_response(),
                        }
                    }

                    // The enum is marked as non-exhaustive
                    _ => (StatusCode::INTERNAL_SERVER_ERROR, headers, message).into_response(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use aws_sdk_s3::error::NoSuchKey;
    use aws_smithy_http::operation::Response as SmithyResponse;
    use aws_smithy_types::error::Error as SmithyError;
    use http::response::Response as HttpResponse;
    use hyper::HeaderMap;

    // Jump through the hoops to get the body as a string.
    async fn body_string(response: Response) -> String {
        String::from_utf8(
            hyper::body::to_bytes(response.into_body())
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap()
    }

    async fn test_active_storage_error(error: ActiveStorageError, status: StatusCode, body: &str) {
        let response = error.into_response();
        assert_eq!(status, response.status());
        let mut headers = HeaderMap::new();
        headers.insert(&header::CONTENT_TYPE, "text/plain".parse().unwrap());
        assert_eq!(headers, *response.headers());
        assert_eq!(body.to_string(), body_string(response).await);
    }

    #[tokio::test]
    async fn empty_array_op_error() {
        let error = ActiveStorageError::EmptyArray { operation: "foo" };
        let body = "cannot perform foo on empty array or selection";
        test_active_storage_error(error, StatusCode::BAD_REQUEST, body).await;
    }

    #[tokio::test]
    async fn from_bytes_error() {
        let error = ActiveStorageError::FromBytes { type_name: "foo" };
        let body = "failed to convert from bytes to foo";
        test_active_storage_error(error, StatusCode::INTERNAL_SERVER_ERROR, body).await;
    }

    #[tokio::test]
    async fn min_max_error() {
        let error = ActiveStorageError::MinMax(MinMaxError::EmptyInput);
        let body = "Empty input.";
        test_active_storage_error(error, StatusCode::BAD_REQUEST, body).await;
    }

    #[tokio::test]
    async fn get_object_error() {
        // Jump through hoops to create an SdkError.
        let smithy_error = SmithyError::builder().message("fake smithy error").build();
        let no_such_key_error = GetObjectErrorKind::NoSuchKey(NoSuchKey::builder().build());
        let get_object_error = GetObjectError::new(no_such_key_error, smithy_error);
        let sdk_body = "body";
        let http_response = HttpResponse::new(sdk_body.into());
        let smithy_response = SmithyResponse::new(http_response);
        let error = ActiveStorageError::S3GetObject(SdkError::service_error(
            get_object_error,
            smithy_response,
        ));
        let body = "fake smithy error";
        test_active_storage_error(error, StatusCode::BAD_REQUEST, body).await;
    }

    #[tokio::test]
    async fn byte_stream_error() {
        // ByteStreamError provides a From impl for std::io:Error.
        let error = ActiveStorageError::S3ByteStream(
            std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into(),
        );
        let body = "IO error";
        test_active_storage_error(error, StatusCode::INTERNAL_SERVER_ERROR, body).await;
    }

    #[tokio::test]
    async fn shape_error() {
        let error = ActiveStorageError::ShapeInvalid(ShapeError::from_kind(
            ndarray::ErrorKind::OutOfBounds,
        ));
        let body = "ShapeError/OutOfBounds: out of bounds indexing";
        test_active_storage_error(error, StatusCode::BAD_REQUEST, body).await;
    }

    #[tokio::test]
    async fn try_from_int_error() {
        let error = ActiveStorageError::TryFromInt(u8::try_from(-1_i8).unwrap_err());
        let body = "out of range integral type conversion attempted";
        test_active_storage_error(error, StatusCode::INTERNAL_SERVER_ERROR, body).await;
    }
}
