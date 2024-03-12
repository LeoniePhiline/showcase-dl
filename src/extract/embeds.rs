use std::sync::Arc;

use color_eyre::eyre::{bail, Result};
use futures::{stream, TryStreamExt};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Url;
use tracing::{debug, info, instrument, trace};

use crate::{state::State, util};

static REGEX_VIDEO_IFRAME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"<iframe[^>]* (?:data-)?src="(?P<embed_url>https://player\.vimeo\.com/video/[^"]+)""#,
    )
    .unwrap()
});

#[instrument(skip(state))]
pub(crate) async fn extract_and_download_embeds(url: Url, state: Arc<State>) -> Result<()> {
    let referer = Some(format!(
        "{}://{}/",
        url.scheme(),
        url.host_str().unwrap_or_default()
    ));

    info!("Fetch source page...");
    state.set_stage_fetching_source(url.as_str()).await;

    let response_text = util::fetch_with_retry(url, None, None)
        .await?
        .text()
        .await?;
    trace!(page_response_text = %response_text);

    info!("Extract embeds...");
    state.set_stage_processing().await;

    tokio::try_join!(
        crate::process::showcase::process_showcases(
            &response_text,
            referer.as_deref(),
            state.clone()
        ),
        process_simple_embeds(&response_text, referer.as_deref(), state.clone())
    )?;

    Ok(())
}

#[instrument(skip(page_body, state))]
async fn process_simple_embeds(
    page_body: &str,
    referer: Option<&str>,
    state: Arc<State>,
) -> Result<()> {
    stream::iter(REGEX_VIDEO_IFRAME.captures_iter(page_body).map(Ok))
        .try_for_each_concurrent(None, |captures| {
            let state = state.clone();
            async move {
                debug!("{captures:#?}");

                match captures.name("embed_url") {
                    Some(embed_url_match) => {
                        let embed_url =
                            htmlize::unescape_attribute(embed_url_match.as_str()).into_owned();

                        crate::process::simple_player::process_simple_player(
                            &embed_url, referer, state,
                        )
                        .await?;

                        Ok(())
                    }
                    None => bail!("Capture group did not match named 'embed_url'"),
                }
            }
        })
        .await?;

    Ok(())
}
