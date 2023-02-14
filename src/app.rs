use aide::{
    axum::{
        routing::{get, post},
        ApiRouter, IntoApiResponse,
    },
    openapi::{Info, OpenApi},
};
use axum::{
    body::Body,
    http::header,
    http::Request,
    http::StatusCode,
    response::IntoResponse,
    Extension,
    Json,
    //routing::{get, post},
    Router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tower_http::validate_request::ValidateRequestHeaderLayer;

pub fn router() -> Router {
    fn v1() -> ApiRouter {
        ApiRouter::new()
            .api_route("/count", post(count))
            //route("/max", post(max))
            //route("/mean", post(mean))
            //route("/min", post(min))
            //route("/select", post(select))
            //route("/sum", post(sum))
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

    let mut api = OpenApi {
        info: Info {
            description: Some("an example API".to_string()),
            ..Info::default()
        },
        ..OpenApi::default()
    };
    ApiRouter::new()
        .route("/.well-known/s3-active-storage-schema", get(schema))
        .nest("/v1", v1())
        .finish_api(&mut api)
        .layer(Extension(api))
}

async fn schema(Extension(api): Extension<OpenApi>) -> impl IntoApiResponse {
    Json(api)
}

#[derive(Deserialize, JsonSchema)]
struct User {
    name: String,
}

async fn count(Json(user): Json<User>) -> impl IntoApiResponse {
    format!("hello {}", user.name)
    //    models::Response::new(
    //        request_data.source.to_string(),
    //        models::DType::Int32,
    //        vec![],
    //    )
}
