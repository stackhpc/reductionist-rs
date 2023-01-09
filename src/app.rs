use crate::models;

use axum::{
    extract::Json,
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

async fn count(Json(request_data): Json<models::RequestData>) -> String {
    format!("Hello, {}!", request_data.source)
}

async fn max(Json(request_data): Json<models::RequestData>) -> String {
    format!("Hello, {}!", request_data.source)
}

async fn mean(Json(request_data): Json<models::RequestData>) -> String {
    format!("Hello, {}!", request_data.source)
}

async fn min(Json(request_data): Json<models::RequestData>) -> String {
    format!("Hello, {}!", request_data.source)
}

async fn select(Json(request_data): Json<models::RequestData>) -> String {
    format!("Hello, {}!", request_data.source)
}

async fn sum(Json(request_data): Json<models::RequestData>) -> String {
    format!("Hello, {}!", request_data.source)
}
