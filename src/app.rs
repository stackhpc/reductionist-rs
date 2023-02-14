use crate::models;
use crate::validated_json::ValidatedJson;

use axum::{
    routing::{get, post},
    Router,
};

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

async fn count(ValidatedJson(request_data): ValidatedJson<models::RequestData>) -> String {
    format!("Hello, {}!", request_data.source)
}

async fn max(ValidatedJson(request_data): ValidatedJson<models::RequestData>) -> String {
    format!("Hello, {}!", request_data.source)
}

async fn mean(ValidatedJson(request_data): ValidatedJson<models::RequestData>) -> String {
    format!("Hello, {}!", request_data.source)
}

async fn min(ValidatedJson(request_data): ValidatedJson<models::RequestData>) -> String {
    format!("Hello, {}!", request_data.source)
}

async fn select(ValidatedJson(request_data): ValidatedJson<models::RequestData>) -> String {
    format!("Hello, {}!", request_data.source)
}

async fn sum(ValidatedJson(request_data): ValidatedJson<models::RequestData>) -> String {
    format!("Hello, {}!", request_data.source)
}
