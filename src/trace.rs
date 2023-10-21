use color_eyre::eyre::{eyre, Result};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{filter::Directive, layer::SubscriberExt};

use crate::args::Args;

pub(crate) fn init(args: &Args) -> Result<WorkerGuard> {
    // TODO: Log into a buffer and display that in a bottom split pane.
    let file_appender = tracing_appender::rolling::never(".", "showcase-dl.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_thread_names(true)
                .with_line_number(true)
                .with_writer(non_blocking)
                .with_filter(
                    // Use `-v` (warn) to `-vvvv` (trace) for simple verbosity,
                    // or use `RUST_LOG=target[span{field=value}]=level` for fine-grained verbosity control.
                    // See https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html
                    tracing_subscriber::EnvFilter::builder()
                        .with_default_directive(
                            LogLevel::from(
                                &args.verbosity.log_level().unwrap_or(log::Level::Error),
                            )
                            .into_directive(),
                        )
                        .from_env_lossy(),
                ),
        )
        .with(ErrorLayer::default())
        .try_init()
        .map_err(|_| eyre!("Tracing initialization failed"))?;

    Ok(guard)
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
