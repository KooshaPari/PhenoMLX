use agileplus_domain::ports::observability::{LogEntry, LogLevel, ObservabilityPort, SpanContext};
use opentelemetry::global;
use opentelemetry::metrics::MeterProvider as _MeterProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use tracing_appender::non_blocking::WorkerGuard;

use crate::{
    config::TelemetryConfig,
    logs::{self, LogError},
    metrics::MetricsRecorder,
};

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("logging init error: {0}")]
    Log(#[from] LogError),
    #[error("config error: {0}")]
    Config(#[from] crate::config::ConfigError),
    #[error("opentelemetry error: {0}")]
    Otel(String),
}

pub struct TelemetryGuard {
    _log_guard: Option<WorkerGuard>,
    _tracer_provider: opentelemetry_sdk::trace::SdkTracerProvider,
    _meter_provider: SdkMeterProvider,
}

pub struct TelemetryAdapter {
    #[allow(dead_code)] // reserved - tracer available for future manual span creation
    tracer: opentelemetry::global::BoxedTracer,
    metrics: MetricsRecorder,
    config: TelemetryConfig,
    _log_guard: Option<WorkerGuard>,
    noop: bool,
}

unsafe impl Send for TelemetryAdapter {}
unsafe impl Sync for TelemetryAdapter {}

impl TelemetryAdapter {
    pub fn new(config: TelemetryConfig) -> Result<Self, TelemetryError> {
        init_trace_provider(&config);

        let meter_provider = SdkMeterProvider::builder().build();
        global::set_meter_provider(meter_provider.clone());

        let meter = global::meter("agileplus");
        let metrics = MetricsRecorder::new(&meter);

        let tracer = global::tracer("agileplus");

        let guard = logs::init_logging(&config.logging).ok();

        Ok(Self {
            tracer,
            metrics,
            config,
            _log_guard: guard,
            noop: false,
        })
    }

    pub fn noop() -> Self {
        let provider = SdkMeterProvider::builder().build();
        let meter = _MeterProvider::meter(&provider, "agileplus-noop");
        let metrics = MetricsRecorder::new(&meter);
        let tracer = opentelemetry::global::tracer("agileplus-noop");

        Self {
            tracer,
            metrics,
            config: TelemetryConfig::default(),
            _log_guard: None,
            noop: true,
        }
    }

    pub fn metrics(&self) -> &MetricsRecorder {
        &self.metrics
    }

    pub fn config(&self) -> &TelemetryConfig {
        &self.config
    }

    pub fn is_noop(&self) -> bool {
        self.noop
    }
}

impl Drop for TelemetryAdapter {
    fn drop(&mut self) {}
}

impl ObservabilityPort for TelemetryAdapter {
    fn start_span(&self, name: &str, parent: Option<&SpanContext>) -> SpanContext {
        if self.noop {
            return noop_span_context();
        }
        let span = match parent {
            Some(p) => tracing::info_span!(
                "agileplus.span",
                name = name,
                parent_span_id = %p.span_id,
                trace_id = %p.trace_id,
            ),
            None => tracing::info_span!("agileplus.span", name = name),
        };

        let span_id = format!("{:?}", span.id().map(|id| id.into_u64()).unwrap_or(0));
        let trace_id = parent
            .map(|p| p.trace_id.clone())
            .unwrap_or_else(|| span_id.clone());

        let _ = span.enter();

        SpanContext {
            trace_id,
            span_id,
            parent_span_id: parent.map(|p| p.span_id.clone()),
        }
    }

    fn end_span(&self, ctx: &SpanContext) {
        if self.noop {
            return;
        }
        tracing::trace!(span_id = %ctx.span_id, "span ended");
    }

    fn add_span_event(&self, ctx: &SpanContext, name: &str, attributes: &[(&str, &str)]) {
        if self.noop {
            return;
        }
        let fields: Vec<String> = attributes.iter().map(|(k, v)| format!("{k}={v}")).collect();
        tracing::info!(
            span_id = %ctx.span_id,
            event = name,
            fields = ?fields,
        );
    }

    fn set_span_error(&self, ctx: &SpanContext, error: &str) {
        if self.noop {
            return;
        }
        tracing::error!(span_id = %ctx.span_id, error = error);
    }

    fn record_counter(&self, name: &str, value: u64, labels: &[(&str, &str)]) {
        if self.noop {
            return;
        }
        let kv: Vec<opentelemetry::KeyValue> = labels
            .iter()
            .map(|(k, v)| opentelemetry::KeyValue::new(k.to_string(), v.to_string()))
            .collect();
        let meter = global::meter("agileplus");
        let counter = meter.u64_counter(name.to_owned()).build();
        counter.add(value, &kv);
    }

    fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        if self.noop {
            return;
        }
        let kv: Vec<opentelemetry::KeyValue> = labels
            .iter()
            .map(|(k, v)| opentelemetry::KeyValue::new(k.to_string(), v.to_string()))
            .collect();
        let meter = global::meter("agileplus");
        let hist = meter.f64_histogram(name.to_owned()).build();
        hist.record(value, &kv);
    }

    fn record_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        if self.noop {
            return;
        }
        let kv: Vec<opentelemetry::KeyValue> = labels
            .iter()
            .map(|(k, v)| opentelemetry::KeyValue::new(k.to_string(), v.to_string()))
            .collect();
        let meter = global::meter("agileplus");
        let gauge = meter.f64_gauge(name.to_owned()).build();
        gauge.record(value, &kv);
    }

    fn log(&self, entry: &LogEntry) {
        if self.noop {
            return;
        }
        let fields_str = format!("{:?}", entry.fields);
        match entry.level {
            LogLevel::Trace => tracing::trace!(message = %entry.message, fields = %fields_str),
            LogLevel::Debug => tracing::debug!(message = %entry.message, fields = %fields_str),
            LogLevel::Info => tracing::info!(message = %entry.message, fields = %fields_str),
            LogLevel::Warn => tracing::warn!(message = %entry.message, fields = %fields_str),
            LogLevel::Error => tracing::error!(message = %entry.message, fields = %fields_str),
        }
    }

    fn log_info(&self, message: &str) {
        if !self.noop {
            tracing::info!("{}", message);
        }
    }

    fn log_warn(&self, message: &str) {
        if !self.noop {
            tracing::warn!("{}", message);
        }
    }

    fn log_error(&self, message: &str) {
        if !self.noop {
            tracing::error!("{}", message);
        }
    }
}

pub fn init_telemetry(config: TelemetryConfig) -> Result<TelemetryGuard, TelemetryError> {
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::trace::SdkTracerProvider;

    let tracer_provider = if let Some(otlp) = &config.otlp {
        match opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&otlp.endpoint)
            .with_timeout(std::time::Duration::from_millis(otlp.timeout_ms))
            .build()
        {
            Ok(exporter) => SdkTracerProvider::builder()
                .with_batch_exporter(exporter)
                .build(),
            Err(e) => {
                tracing::warn!(
                    "OTLP trace provider unavailable ({}): falling back to no-op exporter",
                    e
                );
                SdkTracerProvider::builder().build()
            }
        }
    } else {
        SdkTracerProvider::builder().build()
    };

    global::set_tracer_provider(tracer_provider.clone());

    let meter_provider = SdkMeterProvider::builder().build();
    global::set_meter_provider(meter_provider.clone());

    let guard = logs::init_logging(&config.logging).ok();

    Ok(TelemetryGuard {
        _log_guard: guard,
        _tracer_provider: tracer_provider,
        _meter_provider: meter_provider,
    })
}

fn init_trace_provider(config: &TelemetryConfig) {
    use opentelemetry_sdk::trace::SdkTracerProvider;

    if let Some(otlp) = &config.otlp {
        match build_otlp_provider(otlp) {
            Ok(provider) => {
                global::set_tracer_provider(provider);
                return;
            }
            Err(e) => {
                tracing::warn!(
                    "OTLP trace provider unavailable ({}): falling back to no-op exporter",
                    e
                );
            }
        }
    }

    let provider = SdkTracerProvider::builder().build();
    global::set_tracer_provider(provider);
}

fn build_otlp_provider(
    otlp: &crate::config::OtlpConfig,
) -> Result<opentelemetry_sdk::trace::SdkTracerProvider, String> {
    use opentelemetry_otlp::WithExportConfig;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&otlp.endpoint)
        .with_timeout(std::time::Duration::from_millis(otlp.timeout_ms))
        .build()
        .map_err(|e| e.to_string())?;

    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();

    Ok(provider)
}

fn noop_span_context() -> SpanContext {
    SpanContext {
        trace_id: "00000000000000000000000000000000".into(),
        span_id: "0000000000000000".into(),
        parent_span_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agileplus_domain::ports::observability::LogEntry;
    use std::collections::HashMap;

    #[test]
    fn noop_adapter_does_not_panic() {
        let adapter = TelemetryAdapter::noop();
        assert!(adapter.is_noop());
        adapter.log_info("hello");
        adapter.log_warn("warn");
        adapter.log_error("error");
        let ctx = adapter.start_span("test", None);
        adapter.add_span_event(&ctx, "event", &[("k", "v")]);
        adapter.set_span_error(&ctx, "oops");
        adapter.end_span(&ctx);
        adapter.record_counter("agileplus.test", 1, &[("label", "value")]);
        adapter.record_histogram("agileplus.hist", 42.0, &[]);
        adapter.record_gauge("agileplus.gauge", 1.0, &[]);
    }

    #[test]
    fn noop_adapter_noop_span_context_sentinel() {
        let adapter = TelemetryAdapter::noop();
        let ctx = adapter.start_span("op", None);
        assert_eq!(ctx.trace_id, "00000000000000000000000000000000");
        assert_eq!(ctx.span_id, "0000000000000000");
        assert!(ctx.parent_span_id.is_none());
    }

    #[test]
    fn noop_adapter_log_entry() {
        let adapter = TelemetryAdapter::noop();
        let entry = LogEntry {
            level: LogLevel::Info,
            message: "test".into(),
            fields: HashMap::new(),
            span_context: None,
        };
        adapter.log(&entry);
    }

    #[test]
    fn init_telemetry_returns_guard() {
        let config = TelemetryConfig::default();
        let guard = init_telemetry(config);
        assert!(guard.is_ok());
    }
}
