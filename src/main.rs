use color_eyre::{
    eyre::{bail, eyre, ContextCompat, Result},
    Report,
};
use futures::{stream, TryStreamExt};
use json_dotpath::DotPaths;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::{Client, Url};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info};

mod args;
mod state;
mod trace;
mod util;

use state::{video::Video, State};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let args = args::parse();

    trace::init(&args)?;

    let page_url = Url::parse(&args.url)?;
    debug!("Parsed page URL: {page_url:#?}");

    let referer = format!(
        "{}://{}/",
        page_url.scheme(),
        page_url.host_str().unwrap_or_default()
    );

    info!("Fetch source page...");
    let response_text = Client::new().get(page_url).send().await?.text().await?;
    debug!(page_response_text = ?response_text);

    let state = Arc::new(State::new());

    info!("Extract vimeo embeds...");
    tokio::try_join!(
        process_showcases(&response_text, &referer, state.clone()),
        process_simple_embeds(&response_text, &referer, state.clone())
    )?;

    Ok(())
}

async fn process_simple_embeds(page_body: &str, referer: &str, state: Arc<State>) -> Result<()> {
    lazy_static! {
        static ref RE: Regex = Regex::new(
            r#"<iframe[^>]+ src="(?P<embed_url>https://player\.vimeo\.com/video/[^"]+)""#
        )
        .unwrap();
    }

    stream::iter(RE.captures_iter(page_body).map(Ok))
        .try_for_each_concurrent(None, |captures| {
            let state = state.clone();
            async move {
                debug!("{captures:#?}");

                match captures.name("embed_url") {
                    Some(embed_url_match) => {
                        let embed_url = html_escape::decode_html_entities(embed_url_match.as_str())
                            .into_owned();

                        let video = Arc::new(Video::new(embed_url, referer));
                        (*state).push_video(video.clone()).await;

                        tokio::try_join!(
                            {
                                let video = video.clone();
                                async move {
                                    debug!("Fetch title for simple embed '{}'...", video.url());
                                    extract_simple_embed_title(video, referer).await?;
                                    Ok::<(), Report>(())
                                }
                            },
                            async move {
                                info!("Download simple embed '{}'...", video.url());
                                video.download().await?;
                                Ok(())
                            }
                        )?;

                        Ok(())
                    }
                    None => bail!("Capture group did not match named 'embed_url'"),
                }
            }
        })
        .await?;

    Ok(())
}

async fn extract_simple_embed_title(video: Arc<Video>, referer: &str) -> Result<()> {
    let response_text = util::fetch_with_referer(video.url(), referer).await?;

    lazy_static! {
        static ref RE: Regex = Regex::new(r#"<title>(?P<title>.*?)</title>"#).unwrap();
    }

    let maybe_captures = RE.captures(&response_text);

    if let Some(captures) = maybe_captures {
        if let Some(title_match) = captures.name("title") {
            let matched_title = title_match.as_str();
            debug!(
                "Matched title '{matched_title}' for simple embed '{}'",
                video.url()
            );
            video.update_title(matched_title.into()).await;
        }
    }

    Ok(())
}

async fn process_showcases(page_body: &str, referer: &str, state: Arc<State>) -> Result<()> {
    lazy_static! {
        static ref RE: Regex =
            Regex::new(r#"<iframe[^>]+ src="(?P<embed_url>https://vimeo\.com/showcase/[^"]+)""#)
                .unwrap();
    }

    stream::iter(RE.captures_iter(page_body).map(Ok))
        .try_for_each_concurrent(None, |captures| {
            let referer = &referer;
            let state = state.clone();
            async move {
                debug!("{captures:#?}");

                match captures.name("embed_url") {
                    Some(embed_url_match) => {
                        let embed_url = embed_url_match.as_str();
                        info!("Extract clips from showcase '{embed_url}'...");
                        process_showcase_embed(embed_url, referer, state).await
                    }
                    None => bail!("Capture group did not match named 'embed_url'"),
                }
            }
        })
        .await?;

    Ok(())
}

async fn process_showcase_embed(embed_url: &str, referer: &str, state: Arc<State>) -> Result<()> {
    let response_text = util::fetch_with_referer(embed_url, referer).await?;

    let app_data_line = response_text
        .lines()
        .find(|line| line.contains("app-data"))
        .wrap_err("Script tag 'app-data' not found")?;
    debug!(app_data_line = ?app_data_line);

    let app_data_json = format!(
        "{{{}}}",
        app_data_line
            .split_once('{')
            .wrap_err("Could not front-split 'app-data'")?
            .1
            .rsplit_once('}')
            .wrap_err("Could not back-split 'app-data'")?
            .0
    );
    debug!(app_data_json = ?app_data_json);

    let data: Value = serde_json::from_str(&app_data_json)?;
    debug!(decoded_app_data = ?data);

    // Query for `{ "clips": [...] }` array
    let clips = data.dot_get::<Vec<Value>>("clips")?.ok_or_else(|| {
        eyre!("Could not find 'clips' key in 'app-data', or 'clips' was not an array.")
    })?;
    stream::iter(clips.iter().map(Ok))
        .try_for_each_concurrent(None, |clip| {
            let state = state.clone();
            async move { process_showcase_clip(clip, referer, state).await }
        })
        .await?;

    Ok(())
}

async fn process_showcase_clip(clip: &Value, referer: &str, state: Arc<State>) -> Result<()> {
    let config_url = clip
        .dot_get::<String>("config")?
        .ok_or_else(|| eyre!("Could not read clip config URL from 'app-data.clips.[].config'."))?;

    let client = Client::new();
    let response_text = client.get(config_url).send().await?.text().await?;
    debug!(config_response_text = ?response_text);

    let config: Value = serde_json::from_str(&response_text)?;
    debug!("config response data: {config:#?}");

    let embed_code = config
        .dot_get::<String>("video.embed_code")?
        .ok_or_else(|| {
            eyre!("Could not extract clip embed code 'video.embed_code' from config.")
        })?;

    debug!("config embed_code: {embed_code:#?}");

    lazy_static! {
        static ref RE: Regex = Regex::new(r#"src="(?P<embed_url>[^"]+)""#).unwrap();
    }

    let captures = RE.captures(&embed_code).ok_or_else(|| {
        eyre!(
            "Could not extract embed URL from config 'video.embed_code' string (no regex captures)"
        )
    })?;

    match captures.name("embed_url") {
        Some(embed_url_match) => {
            debug!("embed_url_match: {embed_url_match:#?}");

            let embed_url = html_escape::decode_html_entities(embed_url_match.as_str());
            info!("Download showcase clip '{embed_url}'...");

            let video = Arc::new(Video::new_with_title(
                embed_url,
                referer,
                config.dot_get::<String>("video.title")?, // maybe_title
            ));
            (*state).push_video(video.clone()).await;
            video.download().await?;
        }
        None => {
            bail!("Could not extract embed URL from config 'video.embed_code' string (embed_url not captured)");
        }
    }

    Ok(())
}
