use std::sync::Arc;

use color_eyre::eyre::Result;
use reqwest::Url;
use tracing::{info, instrument};

use crate::state::State;

pub(crate) fn is_player_url(url: &Url) -> bool {
    let host_str = url.host_str().unwrap_or_default();

    host_str.ends_with("vimeo.com")
        || host_str.ends_with("youtube.com")
        || host_str.ends_with("youtu.be")
}

#[instrument(skip(state))]
pub(crate) async fn download_from_player(
    url: Url,
    referer: Option<&str>,
    state: Arc<State>,
) -> Result<()> {
    info!("Download from player...");
    state.set_stage_processing().await;

    let url_str = url.as_str();

    if url_str.starts_with("https://vimeo.com/showcase/") {
        return crate::process::showcase::process_showcase(url_str, referer, state.clone()).await;
    }

    if url_str.starts_with("https://vimeo.com/event/") {
        return crate::process::event::process_event(url_str, state.clone()).await;
        // No referer necessary.
    }

    if url_str.starts_with("https://player.vimeo.com/video/")
        || url_str.starts_with("https://www.youtube.com/watch?v=")
        || url_str.starts_with("https://www.youtube.com/live/")
        || url_str.starts_with("https://youtu.be/")
    {
        return crate::process::simple_player::process_simple_player(
            url_str,
            referer,
            state.clone(),
        )
        .await;
    }

    Ok(())
}
