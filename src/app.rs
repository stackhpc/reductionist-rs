//! Active Storage server API

use crate::app_state::{AppState, SharedAppState};
use crate::cli::CommandLineArgs;
use crate::error::ActiveStorageError;
use crate::filter_pipeline;
use crate::metrics::{metrics_handler, track_metrics};
use crate::models::{self, CBORResponse};
use crate::operation;
use crate::operations;
use crate::validated_json::ValidatedJson;

use axum::middleware;
use axum::{
    extract::{Path, State},
    headers::authorization::{Authorization, Basic},
    http::header,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router, TypedHeader,
};
use bytes::Bytes;
use serde_cbor;
use tower::Layer;
use tower::ServiceBuilder;
use tower_http::normalize_path::NormalizePathLayer;
use tower_http::trace::TraceLayer;
use tracing::debug_span;

impl IntoResponse for models::Response {
    /// Convert a [crate::models::Response] into a [axum::response::Response].
    fn into_response(self) -> Response {
        (
            StatusCode::OK,
            [(&header::CONTENT_TYPE, "application/cbor")],
            serde_cbor::to_vec(&CBORResponse::new(&self))
                .map_err(|e| log::error!("Failed to serialize CBOR: {e}"))
                .unwrap(),
        )
            .into_response()
    }
}

/// Initialise the application
pub fn init(args: &CommandLineArgs) {
    if args.use_rayon {
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_cpus::get() - 1)
            .build_global()
            .expect("Failed to build Rayon thread pool");
    };
}

/// Returns a [axum::Router] for the Active Storage server API
///
/// The router is populated with all routes as well as the following middleware:
///
/// * a [tower_http::trace::TraceLayer] for tracing requests and responses
fn router(args: &CommandLineArgs) -> Router {
    fn v2(state: SharedAppState) -> Router {
        Router::new()
            .route("/count", post(operation_handler::<operations::Count>))
            .route("/max", post(operation_handler::<operations::Max>))
            .route("/min", post(operation_handler::<operations::Min>))
            .route("/select", post(operation_handler::<operations::Select>))
            .route("/sum", post(operation_handler::<operations::Sum>))
            .route("/:operation", post(unknown_operation_handler))
            .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()))
            .with_state(state)
    }

    let state = SharedAppState::new(AppState::new(args));
    Router::new()
        .route("/.well-known/reductionist-schema", get(schema))
        .route("/metrics", get(metrics_handler))
        .nest("/v2", v2(state))
        .route_layer(middleware::from_fn(track_metrics))
}

/// Reductionist Server Service type alias
///
/// This type implements [tower::Service].
// FIXME: The Service type should be some form of tower::Service, but couldn't find the
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
pub fn service(args: &CommandLineArgs) -> Service {
    // Note that any middleware that should affect routing must wrap the router.
    // See
    // https://docs.rs/axum/0.6.18/axum/middleware/index.html#rewriting-request-uri-in-middleware.
    NormalizePathLayer::trim_trailing_slash().layer(router(args))
}

/// TODO: Return an OpenAPI schema
async fn schema() -> &'static str {
    "Hello, world!"
}

/// Handler for Active Storage operations
///
/// Downloads object data and executes the requested reduction operation.
///
/// This function is generic over any type implementing the [crate::operation::Operation] trait,
/// allowing it to handle any operation conforming to that interface.
///
/// Returns a `Result` with [crate::models::Response] on success and
/// [crate::error::ActiveStorageError] on failure.
///
/// # Arguments
///
/// * `auth`: Optional basic authentication header
/// * `request_data`: RequestData object for the request
async fn operation_handler<T: operation::Operation>(
    State(state): State<SharedAppState>,
    auth: Option<TypedHeader<Authorization<Basic>>>,
    ValidatedJson(request_data): ValidatedJson<models::RequestData>,
) -> Result<models::Response, ActiveStorageError> {
    // NOTE(sd109): We acquire memory permits semaphore here so that
    // they are owned by this top-level function and not freed until
    // the permits are dropped when the this function returns.

    // If we're given a size in the request data then use this to
    // get an initial guess at the required memory resources.
    let memory = request_data.size.unwrap_or(0);
    let mem_permits = state.resource_manager.memory(memory).await?;

    // Retrieve the data via the object store which can retrieve the data
    // using the appropriate downloader for the protocol,
    // or return already downloaded and cached data.
    let data = state
        .chunk_store
        .get(&auth, &request_data, &state.resource_manager, mem_permits)
        .await?;

    // All remaining work is synchronous. If the use_rayon argument was specified, delegate to the
    // Rayon thread pool. Otherwise, execute as normal using Tokio.
    if state.args.use_rayon {
        tokio_rayon::spawn(move || operation::<T>(request_data, data)).await
    } else {
        let _task_permit = state.resource_manager.task().await?;
        operation::<T>(request_data, data)
    }
}

/// Perform a reduction operation
///
/// This function encapsulates the synchronous part of an operation.
///
/// # Arguments
///
/// * `request_data`: RequestData object for the request.
/// * `data`: Object data `Bytes`.
fn operation<T: operation::Operation>(
    request_data: models::RequestData,
    data: Bytes,
) -> Result<models::Response, ActiveStorageError> {
    let ptr = data.as_ptr();
    let data = filter_pipeline::filter_pipeline(&request_data, data)?;
    if request_data.compression.is_some() || request_data.size.is_none() {
        // Validate the raw uncompressed data size now that we know it.
        models::validate_raw_size(data.len(), request_data.dtype, &request_data.shape)?;
    }
    if request_data.compression.is_none() && request_data.filters.is_none() {
        // Assert that we're using zero-copy.
        assert_eq!(ptr, data.as_ptr());
    }
    // Convert to a mutable vector to allow in-place byte order conversion.
    let ptr = data.as_ptr();
    let vec: Vec<u8> = data.into();
    // Assert that we're using zero-copy.
    assert_eq!(ptr, vec.as_ptr());
    debug_span!("operation").in_scope(|| T::execute(&request_data, vec))
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
