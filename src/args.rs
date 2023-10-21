use clap::{arg, command, Parser};

pub fn parse() -> Args {
    Args::parse()
}

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    #[command(flatten)]
    pub verbosity: clap_verbosity_flag::Verbosity,

    /// Path to the downloader, such as `yt-dlp` or `youtube-dl`
    #[arg(long, default_value_t = String::from("yt-dlp"))]
    pub downloader: String,

    /// UI refresh interval in milliseconds
    #[arg(short, long, default_value_t = 25)]
    pub tick: u64,

    /// Referer URL - use if passing the URL of a Vimeo showcase or simple player with referer restriction, rather than a page containing embeds
    #[arg(long)]
    pub referer: Option<String>,

    /// URL - Either the target page, containing Vimeo showcase embeds, or a Vimeo showcase URL (with --referer)
    #[arg()]
    pub url: String,

    /// Options passed to the downloader
    #[arg(last = true)]
    pub downloader_options: Vec<String>,
}
