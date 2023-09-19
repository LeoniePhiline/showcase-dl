use std::sync::Arc;

use color_eyre::eyre::{bail, eyre, Result};
use futures::{stream, TryStreamExt};
use json_dotpath::DotPaths;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, info, trace};

use crate::{
    state::{video::Video, State},
    util,
};

static REGEX_SHOWCASE_IFRAME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<iframe[^>]* src="(?P<embed_url>https://vimeo\.com/showcase/[^"]+)""#).unwrap()
});

static REGEX_EMBED_URL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"src="(?P<embed_url>[^"]+)""#).unwrap());

static REGEX_SHOWCASE_CONFIG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"dataForPlayer = (?P<showcase_config>\{.*?\});").unwrap());

pub async fn process_showcases(
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

pub async fn process_showcase(
    showcase_url: &str,
    referer: Option<&str>,
    state: Arc<State>,
) -> Result<()> {
    let response_text = util::fetch_with_referer(showcase_url, referer).await?;

    let maybe_captures = REGEX_SHOWCASE_CONFIG.captures(&response_text);

    if let Some(captures) = maybe_captures {
        if let Some(showcase_config) = captures.name("showcase_config") {
            debug!(
                "Parsing showcase config JSON: {:#?}",
                showcase_config.as_str()
            );
            let data: Value = serde_json::from_str(showcase_config.as_str())?;
            debug!(decoded_showcase_config = ?data);

            // Query for `{ "clips": [...] }` array
            let clips = data.dot_get::<Vec<Value>>("clips")?.ok_or_else(|| {
                eyre!("could not find 'clips' key in 'dataForPlayer', or 'clips' was not an array (hint: if you are passing a Vimeo URL, then try providing the embedding page URL via the '--referer' option)")
            })?;
            stream::iter(clips.into_iter().map(Ok))
                .try_for_each_concurrent(None, |clip| async {
                    let state = state.clone();
                    let referer = referer.map(ToOwned::to_owned);
                    tokio::spawn(async move { process_showcase_clip(&clip, referer, state).await })
                        .await?
                })
                .await?;
        }
    }

    Ok(())
}

async fn process_showcase_clip(
    clip: &Value,
    referer: Option<String>,
    state: Arc<State>,
) -> Result<()> {
    let config_url = clip.dot_get::<String>("config")?.ok_or_else(|| {
        eyre!("could not read clip config URL from 'dataForPlayer.clips.[].config'")
    })?;

    let client = Client::new();
    let response_text = client.get(config_url).send().await?.text().await?;
    trace!(config_response_text = %response_text);

    let config: Value = serde_json::from_str(&response_text)?;
    debug!("config response data: {config:#?}");

    let embed_code = config
        .dot_get::<String>("video.embed_code")?
        .ok_or_else(|| eyre!("could not extract clip embed code 'video.embed_code' from config"))?;

    debug!("config embed_code: {embed_code:#?}");

    let captures = REGEX_EMBED_URL.captures(&embed_code).ok_or_else(|| {
        eyre!(
            "could not extract embed URL from config 'video.embed_code' string (no regex captures)"
        )
    })?;

    match captures.name("embed_url") {
        Some(embed_url_match) => {
            debug!("embed_url_match: {embed_url_match:#?}");

            let embed_url = htmlize::unescape_attribute(embed_url_match.as_str());

            let video = Arc::new(Video::new_with_title(
                embed_url.as_ref(),
                referer,
                config.dot_get::<String>("video.title")?,
            ));
            (*state).push_video(video.clone()).await;

            info!("Download showcase clip '{embed_url}'...");
            video.clone().download(state).await?;
        }
        None => {
            bail!("Could not extract embed URL from config 'video.embed_code' string (embed_url not captured)");
        }
    }

    Ok(())
}
