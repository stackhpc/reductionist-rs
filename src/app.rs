use crate::models;
use crate::s3_client::S3Client;
use crate::validated_json::ValidatedJson;

use axum::{
    body::Body,
    headers::authorization::{Authorization, Basic},
    http::header,
    http::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router, TypedHeader,
};

use tower::ServiceBuilder;
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
            self.result,
        )
            .into_response()
    }
}

pub fn router() -> Router {
    fn v1() -> Router {
        Router::new()
            .route("/count", post(count))
            .route("/max", post(max))
            .route("/mean", post(mean))
            .route("/min", post(min))
            .route("/select", post(select))
            .route("/sum", post(sum))
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
}

async fn schema() -> &'static str {
    "Hello, world!"
}

async fn count(
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ValidatedJson(request_data): ValidatedJson<models::RequestData>,
) -> models::Response {
    let client = S3Client::new(&request_data.source, auth.username(), auth.password()).await;
    let data = client
        .download_object(&request_data.bucket, &request_data.object, None)
        .await;
    let message = format!("{:?}", data);
    models::Response::new(message, models::DType::Int32, vec![])
}

async fn max(
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ValidatedJson(request_data): ValidatedJson<models::RequestData>,
) -> models::Response {
    let message = format!(
        "url {} username {} password {}",
        request_data.source,
        auth.username(),
        auth.password()
    );
    models::Response::new(message, models::DType::Int32, vec![])
}

async fn mean(
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ValidatedJson(request_data): ValidatedJson<models::RequestData>,
) -> models::Response {
    let message = format!(
        "url {} username {} password {}",
        request_data.source,
        auth.username(),
        auth.password()
    );
    models::Response::new(message, models::DType::Int32, vec![])
}

async fn min(
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ValidatedJson(request_data): ValidatedJson<models::RequestData>,
) -> models::Response {
    let message = format!(
        "url {} username {} password {}",
        request_data.source,
        auth.username(),
        auth.password()
    );
    models::Response::new(message, models::DType::Int32, vec![])
}

async fn select(
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ValidatedJson(request_data): ValidatedJson<models::RequestData>,
) -> models::Response {
    let message = format!(
        "url {} username {} password {}",
        request_data.source,
        auth.username(),
        auth.password()
    );
    models::Response::new(message, models::DType::Int32, vec![])
}

async fn sum(
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ValidatedJson(request_data): ValidatedJson<models::RequestData>,
) -> models::Response {
    let message = format!(
        "url {} username {} password {}",
        request_data.source,
        auth.username(),
        auth.password()
    );
    models::Response::new(message, models::DType::Int32, vec![])
}
