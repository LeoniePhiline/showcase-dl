use std::sync::Arc;

use color_eyre::eyre::{eyre, Result};
use json_dotpath::DotPaths;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use tracing::{debug, instrument, trace};

use crate::{state::State, util};

static REGEX_EVENT_URL_PARAMS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https://vimeo.com/event/(?P<event_id>\d+)(?:/(?P<event_hash>[\da-f]+))?").unwrap()
});

#[instrument(skip(state))]
pub(crate) async fn process_event(event_url: &str, state: Arc<State>) -> Result<()> {
    // Assert valid event URL and extract ID and hash.
    let (event_id, maybe_event_hash) = extract_event_url_params(event_url)?;

    // Get event page (input URL), we need the cookie.
    // Reqwest stores the response cookie in its store, to be re-used in `get_jwt`.
    let _response = util::fetch_with_retry(event_url, None, None).await?;

    // Use the cookie to get a JWT.
    let jwt = get_jwt().await?;

    // Use the JWT to retrieve the `clip_to_play` config URL.
    let config_url = retrieve_config_url(event_id, maybe_event_hash, &jwt).await?;

    let share_url = retrieve_share_url(&config_url).await?;

    crate::process::simple_player::process_simple_player(&share_url, None, state).await?;

    Ok(())
}

#[instrument]
fn extract_event_url_params(event_url: &str) -> Result<(&str, Option<&str>)> {
    let captures = REGEX_EVENT_URL_PARAMS
        .captures(event_url)
        .ok_or_else(|| eyre!("'{event_url}' is not a valid event URL"))?;
    let event_id = captures
        .name("event_id")
        .ok_or_else(|| eyre!("no event ID in '{event_url}'"))?;
    let maybe_event_hash = captures
        .name("event_hash")
        .map(|event_hash| event_hash.as_str());

    Ok((event_id.as_str(), maybe_event_hash))
}

#[instrument]
async fn get_jwt() -> Result<String> {
    // Use the cookie to get a JWT.
    let response_text = util::fetch_with_retry("https://vimeo.com/_next/viewer", None, None)
        .await?
        .text()
        .await?;
    trace!(jwt_response_text = %response_text);

    // Parsing in a separate step for easier JSON decode debugging.
    let response_json: Value = serde_json::from_str(&response_text)?;
    debug!("JWT response data: {response_json:#?}");

    let jwt = response_json
        .dot_get::<String>("jwt")?
        .ok_or_else(|| eyre!("could not extract JWT from event viewer data"))?;
    debug!("JWT: {jwt:#?}");

    Ok(jwt)
}

#[instrument]
async fn retrieve_config_url(
    event_id: &str,
    maybe_event_hash: Option<&str>,
    jwt: &str,
) -> Result<String> {
    let response_text = util::fetch_with_retry(
        format!(
            "https://api.vimeo.com/live_events/{event_id}{}?fields=clip_to_play.config_url",
            match maybe_event_hash {
                Some(event_hash) => format!(":{event_hash}"),
                None => String::new(),
            }
        ),
        None,
        Some(&format!("jwt {jwt}")),
    )
    .await?
    .text()
    .await?;
    trace!(live_events_response_text = %response_text);

    // Parsing in a separate step for easier JSON decode debugging.
    let response_json: Value = serde_json::from_str(&response_text)?;
    debug!("live events response data: {response_json:#?}");

    let config_url = response_json
        .dot_get::<String>("clip_to_play.config_url")?
        .ok_or_else(|| {
            eyre!(
                "could not extract video config URL 'clip_to_play.config_url' from live event data"
            )
        })?;
    debug!("Config URL: {config_url:#?}");

    Ok(config_url)
}

#[instrument]
async fn retrieve_share_url(config_url: &str) -> Result<String> {
    let response_text = util::fetch_with_retry(config_url, None, None)
        .await?
        .text()
        .await?;
    trace!(config_response_text = %response_text);

    // Parsing in a separate step for easier JSON decode debugging.
    let response_json: Value = serde_json::from_str(&response_text)?;
    debug!("config response data: {response_json:#?}");

    let share_url = response_json
        .dot_get::<String>("video.share_url")?
        .ok_or_else(|| {
            eyre!("could not extract video share URL 'video.share_url' from config data")
        })?;
    debug!("Config URL: {share_url:#?}");

    Ok(share_url)
}
