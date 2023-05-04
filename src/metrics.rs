use axum::{body::Body, http::Request, response::Response};
use lazy_static::lazy_static;
use prometheus::{self, Encoder, HistogramOpts, HistogramVec, IntCounterVec, Opts};
use tracing::Span;

lazy_static! {
    // Simple request counter
    pub static ref INCOMING_REQUESTS: IntCounterVec = IntCounterVec::new(
        Opts::new("incoming_requests", "The number of HTTP requests received"),
        &["http_method", "path"]
    ).expect("Prometheus metric initialization failed");
    // Request counter by status code
    pub static ref RESPONSE_CODE_COLLECTOR: IntCounterVec = IntCounterVec::new(
        Opts::new("outgoing_response", "The number of responses sent"),
        &["status_code"]
    ).expect("Prometheus metric initialization failed");
    // Response histogram by response time
    pub static ref RESPONSE_TIME_COLLECTOR: HistogramVec = HistogramVec::new(
        HistogramOpts{
            common_opts: Opts::new("response_time", "The time taken to respond to each request"),
            buckets: prometheus::DEFAULT_BUCKETS.to_vec(), // Change buckets here if desired
        },
        &["status_code"],
    ).expect("Prometheus metric initialization failed");
}

/// Registers various prometheus metrics with the global registry
pub fn register_metrics() {
    let registry = prometheus::default_registry();
    registry
        .register(Box::new(INCOMING_REQUESTS.clone()))
        .expect("registering prometheus metrics during initialization failed");
    registry
        .register(Box::new(RESPONSE_CODE_COLLECTOR.clone()))
        .expect("registering prometheus metrics during initialization failed");
    registry
        .register(Box::new(RESPONSE_TIME_COLLECTOR.clone()))
        .expect("registering prometheus metrics during initialization failed");
}

/// Returns currently gathered prometheus metrics
pub async fn metrics_handler() -> String {
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();

    encoder
        .encode(&prometheus::gather(), &mut buffer)
        .expect("could not encode gathered metrics into temporary buffer");

    let output =
        String::from_utf8(buffer.clone()).expect("could not convert metrics buffer into string");
    buffer.clear();

    output
}

/// Gather relevant prometheus metrics on all incoming requests
pub fn record_request_metrics(request: &Request<Body>, _span: &Span) {
    // Increment request counter
    let http_method = &request.method().to_string().to_ascii_uppercase();
    let request_path = &request.uri().path();
    INCOMING_REQUESTS
        .with_label_values(&[http_method, request_path])
        .inc();
}

/// Gather relevant prometheus metrics on all outgoing responses
pub fn record_response_metrics<B>(
    response: &Response<B>,
    latency: std::time::Duration,
    _span: &Span,
) {
    let status_code = response.status();
    // let http_method
    // Record http status code
    RESPONSE_CODE_COLLECTOR
        .with_label_values(&[status_code.as_str()])
        .inc();
    // Record response time
    RESPONSE_TIME_COLLECTOR
        .with_label_values(&[status_code.as_str()])
        .observe(latency.as_secs_f64());
}
