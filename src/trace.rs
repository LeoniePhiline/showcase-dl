use clap_verbosity_flag::Verbosity;
use color_eyre::eyre::{eyre, Result};
use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_error::ErrorLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{prelude::*, EnvFilter};

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
                let tracer = opentelemetry_otlp::new_pipeline()
                    .tracing()
                    .with_exporter(opentelemetry_otlp::new_exporter().http())
                    .with_trace_config(opentelemetry_sdk::trace::config().with_resource(
                        Resource::new(vec![KeyValue::new("service.name", "showcase-dl")]),
                    ))
                    // .install_simple()?;
                    .install_batch(opentelemetry_sdk::runtime::Tokio)?;

                // Create a tracing layer with the configured tracer
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
