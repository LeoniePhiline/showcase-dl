use std::str::FromStr;

use color_eyre::{eyre::Result, Report};
use reqwest::{header::HeaderValue, Client, Url};
use tracing::debug;

pub async fn fetch_with_referer(url: &str, referer: &str) -> Result<String> {
    let referer_header_value = HeaderValue::from_str(referer)?;
    let url = Url::from_str(url)?;

    tokio::spawn(async move {
        let mut referer_header_map = reqwest::header::HeaderMap::new();
        referer_header_map.insert(reqwest::header::REFERER, referer_header_value);

        let response_text = Client::new()
            .get(url)
            .headers(referer_header_map)
            .send()
            .await?
            .text()
            .await?;

        debug!(embed_response_text = ?response_text);

        Ok::<String, Report>(response_text)
    })
    .await?
}
