use axum::{
    routing::post,
    Router,
};

pub fn router() -> Router {
    Router::new()
        .route("/count", post(count))
        .route("/max", post(max))
        .route("/min", post(min))
        .route("/select", post(select))
        .route("/sum", post(sum))
}

async fn count() -> &'static str {
    "Hello, world!"
}

async fn max() -> &'static str {
    "Hello, world!"
}

async fn min() -> &'static str {
    "Hello, world!"
}

async fn select() -> &'static str {
    "Hello, world!"
}

async fn sum() -> &'static str {
    "Hello, world!"
}
