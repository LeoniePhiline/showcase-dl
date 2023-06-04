use std::ops::Deref;

use clap_verbosity_flag::Verbosity;
use color_eyre::eyre::{eyre, Result};
use tracing_appender::non_blocking::WorkerGuard;

use crate::args::Args;

pub fn init(args: &Args) -> Result<WorkerGuard> {
    // TODO: Log into a buffer and display that in a bottom split pane.
    let file_appender = tracing_appender::rolling::never(".", "vimeo-showcase.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .pretty()
        .with_thread_names(true)
        .with_line_number(true)
        .with_max_level(*IntoLevelFilter::from(&args.verbosity))
        .with_writer(non_blocking)
        .try_init()
        .map_err(|_| eyre!("Tracing initialization failed"))?;

    Ok(guard)
}

pub struct IntoLevelFilter(Option<tracing::Level>);

impl From<&Verbosity> for IntoLevelFilter {
    fn from(verbosity: &Verbosity) -> Self {
        Self::from(verbosity.log_level())
    }
}

impl From<Option<log::Level>> for IntoLevelFilter {
    fn from(maybe_log_level: Option<log::Level>) -> Self {
        IntoLevelFilter(maybe_log_level.map(|log_level| *LogLevel::from(log_level)))
    }
}

impl Deref for IntoLevelFilter {
    type Target = Option<tracing::Level>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct LogLevel(tracing::Level);

impl From<log::Level> for LogLevel {
    fn from(log_level: log::Level) -> Self {
        LogLevel(match log_level {
            log::Level::Error => tracing::Level::ERROR,
            log::Level::Warn => tracing::Level::WARN,
            log::Level::Info => tracing::Level::INFO,
            log::Level::Debug => tracing::Level::DEBUG,
            log::Level::Trace => tracing::Level::TRACE,
        })
    }
}

impl Deref for LogLevel {
    type Target = tracing::Level;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
