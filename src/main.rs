use std::sync::Arc;

use color_eyre::{
    eyre::{bail, eyre, Result},
    Report,
};
use futures::{stream, TryStreamExt};
use json_dotpath::DotPaths;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::{Client, Url};
use serde_json::Value;
use tracing::{debug, info, trace};

use state::{video::Video, State};
use ui::Ui;

mod args;
mod state;
mod trace;
mod ui;
mod util;

static REGEX_VIDEO_IFRAME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<iframe[^>]* src="(?P<embed_url>https://player\.vimeo\.com/video/[^"]+)""#)
        .unwrap()
});

static REGEX_TITLE_TAG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"<title>(?P<title>.*?)</title>"#).unwrap());

static REGEX_SHOWCASE_IFRAME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<iframe[^>]* src="(?P<embed_url>https://vimeo\.com/showcase/[^"]+)""#).unwrap()
});

static REGEX_EMBED_URL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"src="(?P<embed_url>[^"]+)""#).unwrap());

static REGEX_SHOWCASE_CONFIG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"dataForPlayer = (?P<showcase_config>\{.*?\});").unwrap());

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre_install()?;

    let args = args::parse();

    let _appender_guard = trace::init(&args)?;

    let state = Arc::new(State::new(args.downloader, args.downloader_options));
    let ui = Ui::new();

    ui.event_loop(state.clone(), args.tick, async move {
        let url = Url::parse(&args.url)?;
        debug!("Parsed page URL: {url:#?}");

        if url.host_str().unwrap_or_default().ends_with("vimeo.com") {
            download_from_player(url, args.referer.as_deref(), state.clone()).await?;
        } else {
            extract_and_download_embeds(url, state.clone()).await?;
        }

        state.set_stage_done().await;

        Ok::<(), Report>(())
    })
    .await?;

    Ok(())
}

async fn download_from_player(url: Url, referer: Option<&str>, state: Arc<State>) -> Result<()> {
    // TODO: Extract this (enclosing) block into a fn
    info!("Extract vimeo embeds...");
    state.set_stage_processing().await;

    if url.as_str().starts_with("https://vimeo.com/showcase/") {
        process_showcase(url.as_str(), referer, state.clone()).await?;
    } else if url.as_str().starts_with("https://player.vimeo.com/video/") {
        process_simple_player(url.as_str(), referer, state.clone()).await?;
    }

    Ok(())
}

async fn extract_and_download_embeds(url: Url, state: Arc<State>) -> Result<()> {
    let referer = Some(format!(
        "{}://{}/",
        url.scheme(),
        url.host_str().unwrap_or_default()
    ));

    info!("Fetch source page...");
    state.set_stage_fetching_source(url.as_str()).await;

    let response_text = Client::new().get(url).send().await?.text().await?;
    trace!(page_response_text = %response_text);

    info!("Extract vimeo embeds...");
    state.set_stage_processing().await;

    tokio::try_join!(
        process_showcases(&response_text, referer.as_deref(), state.clone()),
        process_simple_embeds(&response_text, referer.as_deref(), state.clone())
    )?;

    Ok(())
}

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

                        process_simple_player(&embed_url, referer, state).await?;

                        Ok(())
                    }
                    None => bail!("Capture group did not match named 'embed_url'"),
                }
            }
        })
        .await?;

    Ok(())
}

async fn process_simple_player(
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
            tokio::spawn(async move {
                debug!("Fetch title for simple embed '{}'...", video.url());
                extract_simple_embed_title(video, referer.as_deref()).await?;
                Ok::<(), Report>(())
            })
            .await?
        },
        async {
            let video = video.clone();
            tokio::spawn(async move {
                let url = video.url();
                info!("Download simple embed '{url}'...");
                video.clone().download(state).await?;

                Ok::<(), Report>(())
            })
            .await?
        }
    )?;

    Ok(())
}

async fn extract_simple_embed_title(video: Arc<Video>, referer: Option<&str>) -> Result<()> {
    let response_text = util::fetch_with_referer(video.url(), referer).await?;

    trace!(%response_text, "Trying to extract the video title from '{}'...", video.url());

    let maybe_captures = REGEX_TITLE_TAG.captures(&response_text);

    if let Some(captures) = maybe_captures {
        if let Some(title_match) = captures.name("title") {
            let matched_title = htmlize::unescape(title_match.as_str());
            info!(
                "Matched title '{matched_title}' for simple embed '{}'",
                video.url()
            );
            video.update_title(matched_title.into_owned()).await;
        }
    }

    Ok(())
}

async fn process_showcases(
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

async fn process_showcase(
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
                eyre!("Could not find 'clips' key in 'dataForPlayer', or 'clips' was not an array. If you are passing a Vimeo URL, then try providing the embedding page URL via the '--referer' option.")
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
        eyre!("Could not read clip config URL from 'dataForPlayer.clips.[].config'.")
    })?;

    let client = Client::new();
    let response_text = client.get(config_url).send().await?.text().await?;
    trace!(config_response_text = %response_text);

    let config: Value = serde_json::from_str(&response_text)?;
    debug!("config response data: {config:#?}");

    let embed_code = config
        .dot_get::<String>("video.embed_code")?
        .ok_or_else(|| {
            eyre!("Could not extract clip embed code 'video.embed_code' from config.")
        })?;

    debug!("config embed_code: {embed_code:#?}");

    let captures = REGEX_EMBED_URL.captures(&embed_code).ok_or_else(|| {
        eyre!(
            "Could not extract embed URL from config 'video.embed_code' string (no regex captures)"
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

fn color_eyre_install() -> Result<()> {
    // Replace the default `color_eyre::install()?` panic and error hooks.
    // The new hooks release the captured terminal first. This prevents garbled backtrace prints.
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default().into_hooks();

    // Replace `eyre_hook.install()?`.
    let eyre_hook = eyre_hook.into_eyre_hook();
    color_eyre::eyre::set_hook(Box::new(move |e| {
        let terminal = Ui::make_terminal().expect("make terminal for error handler");
        Ui::release_terminal(terminal).expect("release terminal for error handler");

        eyre_hook(e)
    }))?;

    // Replace `panic_hook.install()`.
    let panic_hook = panic_hook.into_panic_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let terminal = Ui::make_terminal().expect("make terminal for panic handler");
        Ui::release_terminal(terminal).expect("release terminal for panic handler");

        panic_hook(panic_info);
    }));

    Ok(())
}
