use async_trait::async_trait;
use axum::{
    extract::{rejection::JsonRejection, FromRequest, Json},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;
use thiserror::Error;
use validator::Validate;

/// An axum extractor based on the Json extractor that also performs validation using the validator
/// crate.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedJson<T>(pub T);

#[async_trait]
impl<T, S, B> FromRequest<S, B> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
    Json<T>: FromRequest<S, B, Rejection = JsonRejection>,
    B: Send + 'static,
{
    type Rejection = JsonError;

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state).await?;
        value.validate()?;
        Ok(ValidatedJson(value))
    }
}

#[derive(Debug, Error)]
pub enum JsonError {
    #[error(transparent)]
    ValidationError(#[from] validator::ValidationErrors),

    #[error(transparent)]
    AxumJsonRejection(#[from] JsonRejection),
}

/// Return a string describing the error and any sources.
///
/// # Arguments
///
/// * `err`: The error to describe
fn format_error<E: 'static + std::error::Error + Send + Sync>(err: E) -> String {
    let mut message = format!("{}", err);
    let mut current = err.source();
    while let Some(source) = current {
        message += &format!("\nCaused by:\n  {}", source);
        current = source.source();
    }
    message
}

impl IntoResponse for JsonError {
    fn into_response(self) -> Response {
        match self {
            JsonError::ValidationError(_) => {
                let message = format!("Input validation error: [{}]", self).replace('\n', ", ");
                (StatusCode::BAD_REQUEST, message)
            }
            JsonError::AxumJsonRejection(_) => {
                let message = format_error(self);
                (StatusCode::BAD_REQUEST, message)
            }
        }
        .into_response()
    }
}

#[cfg(test)]
mod tests {
    // https://github.com/tokio-rs/axum/blob/main/examples/testing/src/main.rs

    use super::*;
    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
        routing::post,
        Router,
    };
    use regex::Regex;
    use serde::{Deserialize, Serialize};
    use tower::ServiceExt; // for `oneshot` and `ready`

    #[derive(Deserialize, Validate, Serialize)]
    struct TestPayload {
        #[validate(length(min = 1, max = 3))]
        pub foo: String,
        pub bar: Option<u32>,
    }

    // Handler function that accepts a ValidatedJson extractor.
    async fn test_handler(ValidatedJson(payload): ValidatedJson<TestPayload>) -> String {
        format!("foo: {} bar: {:?}", payload.foo, payload.bar)
    }

    // Build a router and make a oneshot request.
    async fn request(body: Body) -> Response {
        Router::new()
            .route("/", post(test_handler))
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(body)
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    // Jump through the hoops to get the body as a string.
    async fn body_string(response: Response) -> String {
        String::from_utf8(
            hyper::body::to_bytes(response.into_body())
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn ok() {
        let body = Body::from("{\"foo\": \"abc\", \"bar\": 123}");
        let response = request(body).await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = body_string(response).await;
        assert_eq!(&body[..], "foo: abc bar: Some(123)");
    }

    #[tokio::test]
    async fn invalid_json() {
        let body = Body::from("{\"");
        let response = request(body).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = body_string(response).await;
        let re = Regex::new(r"Failed to parse the request body as JSON").unwrap();
        assert!(re.is_match(&body[..]))
    }

    #[tokio::test]
    async fn invalid_foo_type() {
        let body = Body::from("{\"foo\": 123}");
        let response = request(body).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = body_string(response).await;
        let re = Regex::new(r".*foo: invalid type: integer `123`.*").unwrap();
        assert!(re.is_match(&body[..]))
    }

    #[tokio::test]
    async fn invalid_foo_too_short() {
        let body = Body::from("{\"foo\": \"\"}");
        let response = request(body).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = body_string(response).await;
        let re =
            Regex::new(r"Input validation error: \[foo: Validation error: length \[\{.*\}\]\]")
                .unwrap();
        assert!(re.is_match(&body[..]))
    }

    #[tokio::test]
    async fn invalid_foo_too_long() {
        let body = Body::from("{\"foo\": \"abcd\"}");
        let response = request(body).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = body_string(response).await;
        let re =
            Regex::new(r"Input validation error: \[foo: Validation error: length \[\{.*\}\]\]")
                .unwrap();
        assert!(re.is_match(&body[..]))
    }
}
