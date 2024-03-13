use clap_verbosity_flag::Verbosity;
use color_eyre::eyre::{eyre, Result};
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_error::ErrorLayer;
use tracing_subscriber::{filter::Directive, layer::SubscriberExt};
use tracing_subscriber::{prelude::*, EnvFilter};

use crate::args::Args;

pub(crate) fn init(args: &Args) -> Result<WorkerGuard> {
    // TODO: Log into a buffer and display that in a bottom split pane.

    // Log file
    let file_appender = tracing_appender::rolling::never(".", "showcase-dl.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let mut metadata = tonic::metadata::MetadataMap::with_capacity(1);
    metadata.insert("x-source-url", args.url.parse()?);
    if let Some(referer) = args.referer.as_deref() {
        metadata.insert("x-referer-url", referer.parse()?);
    }

    // Open telemetry export
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317")
                .with_metadata(metadata),
        )
        .with_trace_config(
            opentelemetry_sdk::trace::config().with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                "showcase-dl",
            )])),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)?;

    // Create a tracing layer with the configured tracer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(telemetry.with_filter(env_filter(&args.verbosity)))
        .with(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_thread_names(true)
                .with_line_number(true)
                .with_writer(non_blocking)
                .with_filter(env_filter(&args.verbosity)),
        )
        .with(ErrorLayer::default())
        .try_init()
        .map_err(|_| eyre!("Tracing initialization failed"))?;

    Ok(guard)
}

fn env_filter(verbosity: &Verbosity) -> EnvFilter {
    // Use `-v` (warn) to `-vvvv` (trace) for simple verbosity,
    // or use `RUST_LOG=target[span{field=value}]=level` for fine-grained verbosity control.
    // See https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html
    tracing_subscriber::EnvFilter::builder()
        .with_default_directive(
            LogLevel::from(&verbosity.log_level().unwrap_or(log::Level::Error)).into_directive(),
        )
        .from_env_lossy()
}

pub(crate) struct LogLevel(tracing::Level);

impl From<&log::Level> for LogLevel {
    fn from(log_level: &log::Level) -> Self {
        LogLevel(match log_level {
            log::Level::Error => tracing::Level::ERROR,
            log::Level::Warn => tracing::Level::WARN,
            log::Level::Info => tracing::Level::INFO,
            log::Level::Debug => tracing::Level::DEBUG,
            log::Level::Trace => tracing::Level::TRACE,
        })
    }
}

impl LogLevel {
    pub(crate) fn into_directive(self) -> Directive {
        self.0.into()
    }
}
