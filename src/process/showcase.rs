use std::sync::Arc;

use color_eyre::eyre::{bail, eyre, Result};
use futures::{stream, TryStreamExt};
use json_dotpath::DotPaths;
use once_cell::sync::Lazy;
use regex::Regex;

use serde_json::Value;
use tracing::{debug, info, instrument, trace, Instrument};

use crate::{
    state::{video::Video, State},
    util,
};

static REGEX_SHOWCASE_IFRAME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<iframe[^>]* (?:data-)?src="(?P<embed_url>https://vimeo\.com/showcase/[^"]+)""#)
        .unwrap()
});

static REGEX_SHOWCASE_CONFIG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\[\{"itemListElement":(?P<showcase_config>\[.*?\]),"@type":"ItemList","@context":"http://schema.org"\}\]"#).unwrap());

#[instrument(skip(page_body, state))]
pub(crate) async fn process_showcases(
    page_body: &str,
    referer: Option<&str>,
    state: Arc<State>,
) -> Result<()> {
    stream::iter(REGEX_SHOWCASE_IFRAME.captures_iter(page_body).map(Ok))
        .try_for_each_concurrent(None, |captures| {
            let state = state.clone();
            async move {
                debug!("{captures:#?}");

                match captures.name("embed_url") {
                    Some(embed_url_match) => {
                        let embed_url = htmlize::unescape_attribute(embed_url_match.as_str());
                        info!("Extract clips from showcase '{embed_url}'...");
                        process_showcase(embed_url.as_ref(), referer, state).await
                    }
                    None => bail!("Capture group did not match named 'embed_url'"),
                }
            }
        })
        .await?;

    Ok(())
}

#[instrument(skip(state))]
pub(crate) async fn process_showcase(
    showcase_url: &str,
    referer: Option<&str>,
    state: Arc<State>,
) -> Result<()> {
    let response_text = util::fetch_with_retry(showcase_url, referer, None)
        .await?
        .text()
        .await?;
    trace!(showcase_response_text = %response_text);

    let maybe_captures = REGEX_SHOWCASE_CONFIG.captures(&response_text);

    let Some(captures) = maybe_captures else {
        bail!(r#"could not find showcase config (`"itemListElement":[...]`) in the HTML response"#)
    };

    if let Some(showcase_config) = captures.name("showcase_config") {
        debug!(
            "Parsing showcase config JSON: {:#?}",
            showcase_config.as_str()
        );
        let clips: Vec<Value> = serde_json::from_str(showcase_config.as_str())?;
        debug!(decoded_showcase_config = ?clips);

        stream::iter(clips.into_iter().map(Ok))
            .try_for_each_concurrent(None, |clip| async {
                let state = state.clone();
                let referer = referer.map(ToOwned::to_owned);
                tokio::spawn(
                    async move { process_showcase_clip(&clip, referer, state).await }
                        .in_current_span(),
                )
                .await?
            })
            .await?;
    }

    Ok(())
}

#[instrument(skip(state))]
async fn process_showcase_clip(
    clip: &Value,
    referer: Option<String>,
    state: Arc<State>,
) -> Result<()> {
    let embed_url = clip.dot_get::<String>("embedUrl")?.ok_or_else(|| {
        eyre!(r#"could not read clip embed URL from '`"itemListElement":[{{..., "embedUrl": "...", ...}}]`"#)
    })?;

    let title = clip.dot_get::<String>("name")?.ok_or_else(|| {
        eyre!(r#"could not read clip title from '`"itemListElement":[{{..., "name": "...", ...}}]`"#)
    })?;


    let video = Arc::new(Video::new_with_title(
        &embed_url,
        referer,
        Some(title),
    ));
    (*state).push_video(video.clone()).await;

    info!("Download showcase clip '{embed_url}'...");
    video.clone().download(state).await?;

    Ok(())
}
