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

    #[clap(flatten)]
    pub verbosity: clap_verbosity_flag::Verbosity,
}
