use std::time::Duration;

use clap_verbosity_flag::Verbosity;
use color_eyre::eyre::{eyre, Result};
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::{Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    runtime::Tokio,
    trace::{
        span_processor_with_async_runtime::BatchSpanProcessor, BatchConfigBuilder,
        SdkTracerProvider,
    },
    Resource,
};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_error::ErrorLayer;
use tracing_subscriber::{layer::SubscriberExt, prelude::*, EnvFilter};

use crate::args::Args;

pub(crate) fn init(args: &Args) -> Result<WorkerGuard> {
    // Log file
    // TODO: Log into a buffer and display that in a bottom split pane.
    let file_appender = tracing_appender::rolling::never(".", "showcase-dl.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(if args.otlp_export {
            Some({
                // Open telemetry export

                // Build resource with service name.
                let resource = Resource::builder_empty()
                    .with_service_name("showcase-dl")
                    .build();

                // Create HTTP exporter with binary protocol.
                let exporter = SpanExporter::builder()
                    .with_http()
                    .with_protocol(Protocol::HttpBinary)
                    .build()?;

                // Create batch span processor with async runtime (Tokio).
                // Configure for faster export: smaller batches (64) and shorter delay (1s)
                let batch_config = BatchConfigBuilder::default()
                    .with_max_export_batch_size(64)
                    .with_scheduled_delay(Duration::from_secs(1))
                    .build();

                let batch_processor = BatchSpanProcessor::builder(exporter, Tokio)
                    .with_batch_config(batch_config)
                    .build();

                // Create tracer provider with the async-aware batch processor.
                let tracer_provider = SdkTracerProvider::builder()
                    .with_span_processor(batch_processor)
                    .with_resource(resource)
                    .build();

                // Get tracer from provider.
                let tracer = tracer_provider.tracer("showcase-dl");

                // Create a tracing layer with the configured tracer.
                let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

                telemetry.with_filter(env_filter(args.verbosity))
            })
        } else {
            None
        })
        .with(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_thread_names(true)
                .with_line_number(true)
                .with_writer(non_blocking)
                .with_filter(env_filter(args.verbosity)),
        )
        .with(ErrorLayer::default())
        .try_init()
        .map_err(|_| eyre!("Tracing initialization failed"))?;

    Ok(guard)
}

fn env_filter(verbosity: Verbosity) -> EnvFilter {
    // Use `-v` (warn) to `-vvvv` (trace) for simple verbosity,
    // or use `RUST_LOG=target[span{field=value}]=level` for fine-grained verbosity control.
    // See https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html
    tracing_subscriber::EnvFilter::builder()
        .with_default_directive(verbosity.tracing_level_filter().into())
        .from_env_lossy()
}
