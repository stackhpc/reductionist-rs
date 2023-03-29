//! Error handling.

use aws_sdk_s3::error::{GetObjectError, GetObjectErrorKind};
use aws_sdk_s3::types::SdkError;
use aws_smithy_http::byte_stream::error::Error as ByteStreamError;
use axum::{
    extract::rejection::JsonRejection,
    http::header,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use ndarray::ShapeError;
use serde::{Deserialize, Serialize};
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

    /// Error deserialising request data into RequestData
    #[error("request data is not valid")]
    RequestDataJsonRejection(#[from] JsonRejection),

    /// Error validating RequestData
    #[error("request data is not valid")]
    RequestDataValidation(#[from] validator::ValidationErrors),

    /// Error reading object data from S3
    #[error("error receiving object from S3 storage")]
    S3ByteStream(#[from] ByteStreamError),

    /// Error while retrieving an object from S3
    #[error("error retrieving object from S3 storage")]
    S3GetObject(#[from] SdkError<GetObjectError>),

    /// Error creating ndarray ArrayView from Shape
    #[error("failed to create array from shape")]
    ShapeInvalid(#[from] ShapeError),

    /// Error converting between integer types
    #[error(transparent)]
    TryFromInt(#[from] std::num::TryFromIntError),

    /// Unsupported operation requested
    #[error("unsupported operation {operation}")]
    UnsupportedOperation { operation: String },
}

// Tell axum how to convert `ActiveStorageError` into a response.
impl IntoResponse for ActiveStorageError {
    fn into_response(self) -> Response {
        ErrorResponse::from(self).into_response()
    }
}

/// Body of error response
///
/// Implements serde (de)serialise.
#[derive(Deserialize, Serialize)]
struct ErrorBody {
    /// Main error message
    message: String,

    /// Optional list of causes
    #[serde(skip_serializing_if = "Option::is_none")]
    caused_by: Option<Vec<String>>,
}

impl ErrorBody {
    /// Return a new ErrorBody
    ///
    /// # Arguments
    ///
    /// * `error`: The error that occurred
    fn new<E>(error: &E) -> Self
    where
        E: std::error::Error + Send + Sync,
    {
        let message = error.to_string();
        let mut caused_by = None;
        let mut current = error.source();
        while let Some(source) = current {
            let mut causes = caused_by.unwrap_or(vec![]);
            causes.push(source.to_string());
            caused_by = Some(causes);
            current = source.source();
        }
        // Remove duplicate entries.
        if let Some(caused_by) = caused_by.as_mut() {
            caused_by.dedup()
        }
        ErrorBody { message, caused_by }
    }
}

/// A response to send in error cases
///
/// Implements serde (de)serialise.
#[derive(Deserialize, Serialize)]
struct ErrorResponse {
    /// HTTP status of the response
    #[serde(skip)]
    status: StatusCode,

    /// Response body
    error: ErrorBody,
}

impl ErrorResponse {
    /// Return a new ErrorResponse
    ///
    /// # Arguments
    ///
    /// * `status`: HTTP status of the response
    /// * `error`: The error that occurred. This will be formatted into a suitable `ErrorBody`
    fn new<E>(status: StatusCode, error: &E) -> Self
    where
        E: std::error::Error + Send + Sync,
    {
        ErrorResponse {
            status,
            error: ErrorBody::new(error),
        }
    }

    /// Return a 400 bad request ErrorResponse
    fn bad_request<E>(error: &E) -> Self
    where
        E: std::error::Error + Send + Sync,
    {
        Self::new(StatusCode::BAD_REQUEST, error)
    }

    /// Return a 404 not found ErrorResponse
    fn not_found<E>(error: &E) -> Self
    where
        E: std::error::Error + Send + Sync,
    {
        Self::new(StatusCode::NOT_FOUND, error)
    }

    /// Return a 500 internal server error ErrorResponse
    fn internal_server_error<E>(error: &E) -> Self
    where
        E: std::error::Error + Send + Sync,
    {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, error)
    }
}

impl From<ActiveStorageError> for ErrorResponse {
    /// Convert from an `ActiveStorageError` into an `ErrorResponse`.
    fn from(error: ActiveStorageError) -> Self {
        match &error {
            // Bad request
            ActiveStorageError::EmptyArray { operation: _ }
            | ActiveStorageError::RequestDataJsonRejection(_)
            | ActiveStorageError::RequestDataValidation(_)
            | ActiveStorageError::ShapeInvalid(_) => Self::bad_request(&error),

            // Not found
            ActiveStorageError::UnsupportedOperation { operation: _ } => Self::not_found(&error),

            // Internal server error
            ActiveStorageError::FromBytes { type_name: _ }
            | ActiveStorageError::TryFromInt(_)
            | ActiveStorageError::S3ByteStream(_) => Self::internal_server_error(&error),

            ActiveStorageError::S3GetObject(sdk_error) => {
                // FIXME: we lose "error retrieving object from S3 storage"
                // Tailor the response based on the specific SdkError variant.
                match &sdk_error {
                    // These are generic SdkError variants.
                    // Internal server error
                    SdkError::ConstructionFailure(_)
                    | SdkError::DispatchFailure(_)
                    | SdkError::ResponseError(_)
                    | SdkError::TimeoutError(_) => Self::internal_server_error(&error),

                    // This is a more specific ServiceError variant, with GetObjectError as the
                    // inner error.
                    SdkError::ServiceError(get_obj_error) => {
                        //let error = if let Some(get_obj_message) = get_obj_error.err().message() {
                        //    // FIXME: use message() & code()?
                        //    &get_obj_error.err()
                        //} else {
                        //    &sdk_error
                        //};
                        let get_obj_error = get_obj_error.err();
                        match get_obj_error.kind {
                            GetObjectErrorKind::InvalidObjectState(_)
                            | GetObjectErrorKind::NoSuchKey(_) => Self::bad_request(&error),

                            // FIXME: Quite a lot of error cases end up here - invalid username,
                            // password, etc.
                            GetObjectErrorKind::Unhandled(_) => Self::internal_server_error(&error),

                            // The enum is marked as non-exhaustive
                            _ => Self::internal_server_error(&error),
                        }
                    }

                    // The enum is marked as non-exhaustive
                    _ => Self::internal_server_error(&error),
                }
            }
        }
    }
}

impl IntoResponse for ErrorResponse {
    /// Convert from an `ErrorResponse` into an `axum::response::Response`.
    ///
    /// Renders the response as JSON.
    fn into_response(self) -> Response {
        let json_body = serde_json::to_string_pretty(&self);
        match json_body {
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to serialise error response: {}", err),
            )
                .into_response(),
            Ok(json_body) => (
                self.status,
                [(&header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())],
                json_body,
            )
                .into_response(),
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

    async fn test_active_storage_error(
        error: ActiveStorageError,
        status: StatusCode,
        message: &str,
        caused_by: Option<Vec<&'static str>>,
    ) {
        let response = error.into_response();
        assert_eq!(status, response.status());
        let mut headers = HeaderMap::new();
        headers.insert(&header::CONTENT_TYPE, "application/json".parse().unwrap());
        assert_eq!(headers, *response.headers());
        let error_response: ErrorResponse =
            serde_json::from_str(&body_string(response).await).unwrap();
        assert_eq!(message.to_string(), error_response.error.message);
        // Map Vec items from str to String
        let caused_by = caused_by.map(|cb| cb.iter().map(|s| s.to_string()).collect());
        assert_eq!(caused_by, error_response.error.caused_by);
    }

    #[tokio::test]
    async fn empty_array_op_error() {
        let error = ActiveStorageError::EmptyArray { operation: "foo" };
        let message = "cannot perform foo on empty array or selection";
        let caused_by = None;
        test_active_storage_error(error, StatusCode::BAD_REQUEST, message, caused_by).await;
    }

    #[tokio::test]
    async fn from_bytes_error() {
        let error = ActiveStorageError::FromBytes { type_name: "foo" };
        let message = "failed to convert from bytes to foo";
        let caused_by = None;
        test_active_storage_error(error, StatusCode::INTERNAL_SERVER_ERROR, message, caused_by)
            .await;
    }

    #[tokio::test]
    async fn request_data_validation() {
        let mut validation_errors = validator::ValidationErrors::new();
        let validation_error = validator::ValidationError::new("foo");
        validation_errors.add("bar", validation_error);
        let error = ActiveStorageError::RequestDataValidation(validation_errors);
        let message = "request data is not valid";
        let caused_by = Some(vec!["bar: Validation error: foo [{}]"]);
        test_active_storage_error(error, StatusCode::BAD_REQUEST, message, caused_by).await;
    }

    #[tokio::test]
    async fn s3_get_object_error() {
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
        let message = "error retrieving object from S3 storage";
        let caused_by = Some(vec!["service error", "NoSuchKey"]);
        test_active_storage_error(error, StatusCode::BAD_REQUEST, message, caused_by).await;
    }

    #[tokio::test]
    async fn s3_byte_stream_error() {
        // ByteStreamError provides a From impl for std::io:Error.
        let error = ActiveStorageError::S3ByteStream(
            std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into(),
        );
        let message = "error receiving object from S3 storage";
        let caused_by = Some(vec!["IO error", "unexpected end of file"]);
        test_active_storage_error(error, StatusCode::INTERNAL_SERVER_ERROR, message, caused_by)
            .await;
    }

    #[tokio::test]
    async fn shape_error() {
        let error = ActiveStorageError::ShapeInvalid(ShapeError::from_kind(
            ndarray::ErrorKind::OutOfBounds,
        ));
        let message = "failed to create array from shape";
        let caused_by = Some(vec!["ShapeError/OutOfBounds: out of bounds indexing"]);
        test_active_storage_error(error, StatusCode::BAD_REQUEST, message, caused_by).await;
    }

    #[tokio::test]
    async fn try_from_int_error() {
        let error = ActiveStorageError::TryFromInt(u8::try_from(-1_i8).unwrap_err());
        let message = "out of range integral type conversion attempted";
        let caused_by = None;
        test_active_storage_error(error, StatusCode::INTERNAL_SERVER_ERROR, message, caused_by)
            .await;
    }

    #[tokio::test]
    async fn unsupported_operation() {
        let error = ActiveStorageError::UnsupportedOperation {
            operation: "foo".to_string(),
        };
        let message = "unsupported operation foo";
        let caused_by = None;
        test_active_storage_error(error, StatusCode::NOT_FOUND, message, caused_by).await;
    }
}