use color_eyre::eyre::Result;
use reqwest::{header::HeaderValue, Client};
use tracing::debug;

pub async fn fetch_with_referer(url: &str, referer: &str) -> Result<String> {
    let mut referer_header_map = reqwest::header::HeaderMap::new();
    referer_header_map.insert(reqwest::header::REFERER, HeaderValue::from_str(referer)?);

    let response_text = Client::new()
        .get(url)
        .headers(referer_header_map)
        .send()
        .await?
        .text()
        .await?;
    debug!(embed_response_text = ?response_text);
    Ok(response_text)
}
