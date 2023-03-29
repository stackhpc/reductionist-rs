use crate::error::ActiveStorageError;
use crate::models;
use crate::operation;
use crate::operations;
use crate::s3_client;
use crate::validated_json::ValidatedJson;

use axum::{
    body::{Body, Bytes},
    extract::Path,
    headers::authorization::{Authorization, Basic},
    http::header,
    http::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router, TypedHeader,
};

use tower::ServiceBuilder;
use tower_http::normalize_path::NormalizePathLayer;
use tower_http::trace::TraceLayer;
use tower_http::validate_request::ValidateRequestHeaderLayer;

static HEADER_DTYPE: header::HeaderName = header::HeaderName::from_static("x-activestorage-dtype");
static HEADER_SHAPE: header::HeaderName = header::HeaderName::from_static("x-activestorage-shape");

impl IntoResponse for models::Response {
    fn into_response(self) -> Response {
        (
            [
                (
                    &header::CONTENT_TYPE,
                    mime::APPLICATION_OCTET_STREAM.to_string(),
                ),
                (&HEADER_DTYPE, self.dtype.to_string().to_lowercase()),
                (&HEADER_SHAPE, serde_json::to_string(&self.shape).unwrap()),
            ],
            self.body,
        )
            .into_response()
    }
}

pub fn router() -> Router {
    fn v1() -> Router {
        Router::new()
            .route("/count", post(operation_handler::<operations::Count>))
            .route("/max", post(operation_handler::<operations::Max>))
            .route("/mean", post(operation_handler::<operations::Mean>))
            .route("/min", post(operation_handler::<operations::Min>))
            .route("/select", post(operation_handler::<operations::Select>))
            .route("/sum", post(operation_handler::<operations::Sum>))
            .route("/:operation", post(unknown_operation_handler))
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(ValidateRequestHeaderLayer::custom(
                        // Validate that an authorization header has been provided.
                        |request: &mut Request<Body>| {
                            if request.headers().contains_key(header::AUTHORIZATION) {
                                Ok(())
                            } else {
                                Err(StatusCode::UNAUTHORIZED.into_response())
                            }
                        },
                    )),
            )
    }

    Router::new()
        .route("/.well-known/s3-active-storage-schema", get(schema))
        .nest("/v1", v1())
        .layer(NormalizePathLayer::trim_trailing_slash())
}

async fn schema() -> &'static str {
    "Hello, world!"
}

async fn download_object(
    auth: &Authorization<Basic>,
    request_data: &models::RequestData,
) -> Result<Bytes, ActiveStorageError> {
    let range = s3_client::get_range(request_data.offset, request_data.size);
    s3_client::S3Client::new(&request_data.source, auth.username(), auth.password())
        .await
        .download_object(&request_data.bucket, &request_data.object, range)
        .await
}

/// Handler for operations
///
/// Returns a `Result` with `models::Response` on success and `ActiveStorageError` on failure.
///
/// # Arguments
///
/// * `auth`: Basic authorization header
/// * `request_data`: RequestData object for the request
async fn operation_handler<T: operation::Operation>(
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ValidatedJson(request_data): ValidatedJson<models::RequestData>,
) -> Result<models::Response, ActiveStorageError> {
    let data = download_object(&auth, &request_data).await?;
    T::execute(&request_data, &data)
}

async fn unknown_operation_handler(Path(operation): Path<String>) -> ActiveStorageError {
    ActiveStorageError::UnsupportedOperation { operation }
}
