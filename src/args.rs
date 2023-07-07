use clap::{arg, command, Parser};

pub fn parse() -> Args {
    Args::parse()
}

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    #[command(flatten)]
    pub verbosity: clap_verbosity_flag::Verbosity,

    #[arg(long, default_value_t = String::from("yt-dlp"))]
    pub downloader: String,

    /// UI refresh interval in milliseconds
    #[arg(short, long, default_value_t = 50)]
    pub tick: u64,

    /// URL of the target page, containing Vimeo showcase embeds
    #[arg()]
    pub url: String,

    /// Options passed to the downloader
    #[arg(last = true)]
    pub downloader_options: Vec<String>,
}
