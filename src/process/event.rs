use std::sync::Arc;

use color_eyre::{
    eyre::{eyre, Result},
};
use json_dotpath::DotPaths;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::{header::AUTHORIZATION, Client, ClientBuilder};
use serde_json::Value;
use tracing::{debug, trace};

use crate::{
    state::{State},
};

static REGEX_EVENT_URL_PARAMS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https://vimeo.com/event/(?P<event_id>\d+)/(?P<event_hash>[\da-f]+)").unwrap()
});

pub async fn process_event(event_url: &str, state: Arc<State>) -> Result<()> {
    // Assert valid event URL and extract ID and hash.
    let (event_id, event_hash) = extract_event_url_params(event_url)?;

    // Enable the cookie store feature.
    let client = ClientBuilder::new().cookie_store(true).build()?;

    // Get event page (input URL), we need the cookie.
    // Reqwest stores the response cookie in its store, to be re-used in `get_jwt`.
    let _response = client.get(event_url).send().await?;

    // Use the cookie to get a JWT.
    let jwt = get_jwt(client.clone()).await?;

    // Use the JWT to retrieve the `streamable_clip` config URL.
    let config_url = retrieve_config_url(event_id, event_hash, &jwt).await?;

    let share_url = retrieve_share_url(&config_url).await?;

    crate::process::simple_player::process_simple_player(&share_url, None, state).await?;

    Ok(())
}

fn extract_event_url_params(event_url: &str) -> Result<(&str, &str)> {
    let captures = REGEX_EVENT_URL_PARAMS
        .captures(event_url)
        .ok_or_else(|| eyre!("'{event_url}' is not a valid event URL"))?;
    let event_id = captures
        .name("event_id")
        .ok_or_else(|| eyre!("no event ID in '{event_url}'"))?;
    let event_hash = captures
        .name("event_hash")
        .ok_or_else(|| eyre!("no event hash in '{event_url}'"))?;

    Ok((event_id.as_str(), event_hash.as_str()))
}

async fn get_jwt(authenticated_client: Client) -> Result<String> {
    // Use the cookie to get a JWT.
    let response_text = authenticated_client
        .get("https://vimeo.com/_next/viewer")
        .send()
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

async fn retrieve_config_url(event_id: &str, event_hash: &str, jwt: &str) -> Result<String> {
    let response_text = Client::new().get(format!("https://api.vimeo.com/live_events/{event_id}:{event_hash}?fields=streamable_clip.config_url")).header(AUTHORIZATION, format!("jwt {jwt}")).send().await?.text().await?;
    trace!(live_events_response_text = %response_text);

    // Parsing in a separate step for easier JSON decode debugging.
    let response_json: Value = serde_json::from_str(&response_text)?;
    debug!("live events response data: {response_json:#?}");

    let config_url = response_json.dot_get::<String>("streamable_clip.config_url")?.ok_or_else(|| eyre!("could not extract video config URL 'streamable_clip.config_url' from live event data"))?;
    debug!("Config URL: {config_url:#?}");

    Ok(config_url)
}

async fn retrieve_share_url(config_url: &str) -> Result<String> {
    let response_text = Client::new().get(config_url).send().await?.text().await?;
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