use clap::{arg, command, Parser};

pub(crate) fn parse() -> Args {
    Args::parse()
}

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub(crate) struct Args {
    /// Path to the downloader, such as `yt-dlp` or `youtube-dl`
    #[arg(long, default_value_t = String::from("yt-dlp"))]
    pub(crate) downloader: String,

    /// Export OTLP traces - run a trace collector such as jaeger when using this option
    #[arg(long)]
    pub(crate) otlp_export: bool,

    /// Referer URL - use if passing the URL of a Vimeo showcase or simple player with referer restriction, rather than a page containing embeds
    #[arg(long)]
    pub(crate) referer: Option<String>,

    /// UI refresh interval in milliseconds
    #[arg(short, long, default_value_t = 25)]
    pub(crate) tick: u64,

    #[command(flatten)]
    pub(crate) verbosity: clap_verbosity_flag::Verbosity,

    /// URL - Either the target page, containing Vimeo showcase embeds, or a Vimeo showcase URL (with --referer)
    #[arg()]
    pub(crate) url: String,

    /// Options passed to the downloader
    #[arg(last = true)]
    pub(crate) downloader_options: Vec<String>,
}
