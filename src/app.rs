use crate::models;

use axum::{
    extract::Json,
    routing::post,
    Router,
};

pub fn router() -> Router {
    Router::new()
        .route("/count", post(count))
        .route("/max", post(max))
        .route("/mean", post(mean))
        .route("/min", post(min))
        .route("/select", post(select))
        .route("/sum", post(sum))
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
