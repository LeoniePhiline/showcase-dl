use std::sync::Arc;

use color_eyre::{eyre::Result, Report};
use reqwest::Url;
use tracing::debug;

use crate::state::State;
use crate::ui::Ui;

mod args;
mod error;
mod extract;
mod process;
mod state;
mod trace;
mod ui;
mod util;

#[tokio::main]
async fn main() -> Result<()> {
    error::color_eyre_install()?;

    let args = args::parse();

    let _appender_guard = trace::init(&args)?;

    let state = Arc::new(State::new(args.downloader, args.downloader_options));
    let ui = Ui::new();

    ui.event_loop(state.clone(), args.tick, async move {
        let url = Url::parse(&args.url)?;
        debug!("Parsed page URL: {url:#?}");

        if extract::player::is_player_url(&url) {
            extract::player::download_from_player(url, args.referer.as_deref(), state.clone())
                .await?;
        } else {
            extract::embeds::extract_and_download_embeds(url, state.clone()).await?;
        }

        state.set_stage_done().await;

        Ok::<(), Report>(())
    })
    .await?;

    Ok(())
}
