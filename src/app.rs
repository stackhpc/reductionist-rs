//! Active Storage server API

use crate::error::ActiveStorageError;
use crate::filter_pipeline;
use crate::metrics::{metrics_handler, track_metrics};
use crate::models;
use crate::operation;
use crate::operations;
use crate::s3_client;
use crate::validated_json::ValidatedJson;

use axum::middleware;
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

use tower::Layer;
use tower::ServiceBuilder;
use tower_http::normalize_path::NormalizePathLayer;
use tower_http::trace::TraceLayer;
use tower_http::validate_request::ValidateRequestHeaderLayer;

/// `x-activestorage-dtype` header definition
static HEADER_DTYPE: header::HeaderName = header::HeaderName::from_static("x-activestorage-dtype");
/// `x-activestorage-shape` header definition
static HEADER_SHAPE: header::HeaderName = header::HeaderName::from_static("x-activestorage-shape");
/// `x-activestorage-count` header definition
static HEADER_COUNT: header::HeaderName = header::HeaderName::from_static("x-activestorage-count");

impl IntoResponse for models::Response {
    /// Convert a [crate::models::Response] into a [axum::response::Response].
    fn into_response(self) -> Response {
        (
            [
                (
                    &header::CONTENT_TYPE,
                    mime::APPLICATION_OCTET_STREAM.to_string(),
                ),
                (&HEADER_DTYPE, self.dtype.to_string().to_lowercase()),
                (&HEADER_SHAPE, serde_json::to_string(&self.shape).unwrap()),
                (&HEADER_COUNT, serde_json::to_string(&self.count).unwrap()),
            ],
            self.body,
        )
            .into_response()
    }
}

/// Returns a [axum::Router] for the Active Storage server API
///
/// The router is populated with all routes as well as the following middleware:
///
/// * a [tower_http::trace::TraceLayer] for tracing requests and responses
/// * a [tower_http::validate_request::ValidateRequestHeaderLayer] for validating authorisation
///   headers
fn router() -> Router {
    fn v1() -> Router {
        Router::new()
            .route("/count", post(operation_handler::<operations::Count>))
            .route("/max", post(operation_handler::<operations::Max>))
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
        .route("/metrics", get(metrics_handler))
        .nest("/v1", v1())
        .route_layer(middleware::from_fn(track_metrics))
}

/// S3 Active Storage Server Service type alias
///
/// This type implements [tower_service::Service].
// FIXME: The Service type should be some form of tower_service::Service, but couldn't find the
// necessary trait bounds.
pub type Service = tower_http::normalize_path::NormalizePath<Router>;

/// Returns a [crate::app::Service] for the Active Storage server API
///
/// The service is populated with all routes as well as the following middleware:
///
/// * a [tower_http::trace::TraceLayer] for tracing requests and responses
/// * a [tower_http::validate_request::ValidateRequestHeaderLayer] for validating authorisation
///   headers
/// * a [tower_http::normalize_path::NormalizePathLayer] for trimming trailing slashes from
///   requests
pub fn service() -> Service {
    // Note that any middleware that should affect routing must wrap the router.
    // See
    // https://docs.rs/axum/0.6.18/axum/middleware/index.html#rewriting-request-uri-in-middleware.
    NormalizePathLayer::trim_trailing_slash().layer(router())
}

/// TODO: Return an OpenAPI schema
async fn schema() -> &'static str {
    "Hello, world!"
}

/// Download an object from S3
///
/// Requests a byte range if `offset` or `size` is specified in the request.
///
/// # Arguments
///
/// * `auth`: Basic authentication credentials
/// * `request_data`: RequestData object for the request
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

/// Handler for Active Storage operations
///
/// Downloads object data from S3 storage and executes the requested reduction operation.
///
/// This function is generic over any type implementing the [crate::operation::Operation] trait,
/// allowing it to handle any operation conforming to that interface.
///
/// Returns a `Result` with [crate::models::Response] on success and
/// [crate::error::ActiveStorageError] on failure.
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
    let data = filter_pipeline::filter_pipeline(&request_data, &data)?;
    if request_data.compression.is_some() || request_data.size.is_none() {
        // Validate the raw uncompressed data size now that we know it.
        models::validate_raw_size(data.len(), request_data.dtype, &request_data.shape)?;
    }
    T::execute(&request_data, &data)
}

/// Handler for unknown operations
///
/// Returns an [crate::error::ActiveStorageError].
///
/// # Arguments
///
/// * `operation`: the unknown operation from the URL path
async fn unknown_operation_handler(Path(operation): Path<String>) -> ActiveStorageError {
    ActiveStorageError::UnsupportedOperation { operation }
}
