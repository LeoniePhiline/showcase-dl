use std::sync::Arc;

use color_eyre::{eyre::Result, Report};
use once_cell::sync::Lazy;
use regex::Regex;
use tracing::{debug, info, instrument, trace, Instrument};

use crate::{
    state::{video::Video, State},
    util,
};

static REGEX_TITLE_TAG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<title>(?P<title>.*?)</title>").unwrap());

#[instrument(skip(state))]
pub(crate) async fn process_simple_player(
    player_url: &str,
    referer: Option<&str>,
    state: Arc<State>,
) -> Result<()> {
    let video = Arc::new(Video::new(player_url, referer));
    (*state).push_video(video.clone()).await;

    tokio::try_join!(
        async {
            let video = video.clone();
            let referer = referer.map(ToOwned::to_owned);
            tokio::spawn(
                async move {
                    debug!("Fetch title for simple player '{}'...", video.url());
                    extract_simple_player_title(video, referer.as_deref()).await?;
                    Ok::<(), Report>(())
                }
                .in_current_span(),
            )
            .await?
        },
        async {
            let video = video.clone();
            tokio::spawn(
                async move {
                    let url = video.url();
                    info!("Download simple player '{url}'...");
                    video.clone().download(state).await?;

                    Ok::<(), Report>(())
                }
                .in_current_span(),
            )
            .await?
        }
    )?;

    Ok(())
}

#[instrument]
async fn extract_simple_player_title(video: Arc<Video>, referer: Option<&str>) -> Result<()> {
    let response_text = util::fetch_with_retry(video.url(), referer, None)
        .await?
        .text()
        .await?;

    trace!(%response_text, "Trying to extract the video title from '{}'...", video.url());

    let maybe_captures = REGEX_TITLE_TAG.captures(&response_text);

    if let Some(captures) = maybe_captures {
        if let Some(title_match) = captures.name("title") {
            let matched_title = htmlize::unescape(title_match.as_str());
            info!(
                "Matched title '{matched_title}' for simple player '{}'",
                video.url()
            );
            video.update_title(matched_title.into_owned()).await;
        }
    }

    Ok(())
}
