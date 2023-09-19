use std::sync::Arc;

use color_eyre::eyre::Result;
use reqwest::Url;
use tracing::info;

use crate::state::State;

pub async fn download_from_player(
    url: Url,
    referer: Option<&str>,
    state: Arc<State>,
) -> Result<()> {
    info!("Extract vimeo embeds...");
    state.set_stage_processing().await;

    if url.as_str().starts_with("https://vimeo.com/showcase/") {
        crate::process::showcase::process_showcase(url.as_str(), referer, state.clone()).await?;
    } else if url.as_str().starts_with("https://player.vimeo.com/video/") {
        crate::process::simple_player::process_simple_player(url.as_str(), referer, state.clone())
            .await?;
    } else if url.as_str().starts_with("https://vimeo.com/event/") {
        crate::process::event::process_event(url.as_str(), state.clone()).await?;
        // No referer necessary.
    }

    Ok(())
}
