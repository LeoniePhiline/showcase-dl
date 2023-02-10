use clap::{arg, command, Parser};

pub fn parse() -> Args {
    Args::parse()
}

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    /// URL of the target page, containing Vimeo showcase embeds
    pub url: String,

    /// UI refresh interval in milliseconds
    #[arg(short, long, default_value_t = 50)]
    pub tick: u64,

    #[arg(long, default_value_t = String::from("yt-dlp"))]
    pub bin: String,

    #[command(flatten)]
    pub verbosity: clap_verbosity_flag::Verbosity,
}
