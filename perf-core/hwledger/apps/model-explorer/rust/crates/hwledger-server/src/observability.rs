// SPDX-License-Identifier: Apache-2.0
//! Structured tracing spans for the Model Explorer HTTP routes.
//!
//! The `tracing` crate already gives us per-`Span` timing and structured
//! fields; we just need to opt the route handlers in. Each instrumented
//! handler emits a span on entry with:
//!
//!   * `path`            — the matched route path (filled in by middleware)
//!   * `handler`         — handler fn name (auto from `#[instrument]`)
//!   * `model_id` / `q`  — request-specific fields when relevant
//!
//! The span carries through to all the `service::search_results` /
//! `service::detail` / `service::quants` / etc. calls inside the
//! handler, so a single grep `tracing::span!` / `tracing::info!` in
//! any downstream code is attributed to the request.
//!
//! We deliberately do NOT pull in the `opentelemetry` crate. It's a
//! major dep that hits PointeeSized issues on Rust 1.99 with the
//! version that ships with cargo today. The `tracing` ecosystem is
//! the standard Rust observability layer; an `opentelemetry_otlp`
//! exporter can be layered on later if/when a stable otel release
//! catches up.

use axum::{
    body::Body,
    extract::Request,
    middleware::Next,
    response::Response,
};

use tracing::Instrument;

/// Per-request middleware that attaches `path` to the current span.
/// Combined with `#[tracing::instrument]` on each handler, this gives
/// every log line emitted from a request handler the matched route.
pub async fn attach_path_to_span(req: Request<Body>, next: Next) -> Response {
    let path = req
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "<unknown>".to_string());

    let method = req.method().clone();
    let span = tracing::info_span!("request", %method, path);

    let response = next.run(req).instrument(span.clone()).await;

    // Emit an access-log line on the way out so the operator can see
    // the per-request summary without needing a tracing subscriber
    // configured for span output.
    let status = response.status().as_u16();
    tracing::info!(status, %method, path = %path, "request complete");

    response
}
