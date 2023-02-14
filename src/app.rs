use crate::models;
use crate::validated_json::ValidatedJson;

use axum::{
    http::header,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};

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
    }

    Router::new()
        .route("/.well-known/s3-active-storage-schema", get(schema))
        .nest("/v1", v1())
}

async fn schema() -> &'static str {
    "Hello, world!"
}

async fn count(
    ValidatedJson(request_data): ValidatedJson<models::RequestData>,
) -> models::Response {
    models::Response::new(
        request_data.source.to_string(),
        models::DType::Int32,
        vec![],
    )
}

async fn max(ValidatedJson(request_data): ValidatedJson<models::RequestData>) -> models::Response {
    models::Response::new(
        request_data.source.to_string(),
        models::DType::Int32,
        vec![],
    )
}

async fn mean(ValidatedJson(request_data): ValidatedJson<models::RequestData>) -> models::Response {
    models::Response::new(
        request_data.source.to_string(),
        models::DType::Int32,
        vec![],
    )
}

async fn min(ValidatedJson(request_data): ValidatedJson<models::RequestData>) -> models::Response {
    models::Response::new(
        request_data.source.to_string(),
        models::DType::Int32,
        vec![],
    )
}

async fn select(
    ValidatedJson(request_data): ValidatedJson<models::RequestData>,
) -> models::Response {
    models::Response::new(
        request_data.source.to_string(),
        models::DType::Int32,
        vec![],
    )
}

async fn sum(ValidatedJson(request_data): ValidatedJson<models::RequestData>) -> models::Response {
    models::Response::new(
        request_data.source.to_string(),
        models::DType::Int32,
        vec![],
    )
}
