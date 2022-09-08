use clap::Parser;

pub fn parse() -> Args {
    Args::parse()
}

#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub struct Args {
    /// URL of the target page, containing Vimeo showcase embeds
    #[clap(action)]
    pub url: String,

    /// UI refresh interval in milliseconds
    #[clap(short, long, default_value_t = 50, action)]
    pub tick: u64,

    #[clap(flatten)]
    pub verbosity: clap_verbosity_flag::Verbosity,
}
