use std::str::FromStr;

use color_eyre::{eyre::Result, Report};
use reqwest::{
    header::{HeaderValue, REFERER, USER_AGENT},
    Client, Url,
};
use tokio::task::JoinHandle;
use tracing::trace;

// Fetch a URL, applying a referer header
pub async fn fetch_with_referer(url: &str, referer: Option<&str>) -> Result<String> {
    let referer_header_value = match referer {
        Some(referer) => HeaderValue::from_str(referer).map(Some),
        None => Ok(None),
    }?;
    let url = Url::from_str(url)?;

    tokio::spawn(async move {
        let mut request = Client::new()
            .get(url)
            .header(
                USER_AGENT,
                "Mozilla/5.0 (X11; U; Linux x86_64; en-US; rv:115.0esr) Gecko/20110619 Firefox/115.0esr",
            );
        
        if let Some(referer_header_value) = referer_header_value {
            request = request.header(REFERER, referer_header_value);
        }

        let response_text = request
            .send()
            .await?
            .text()
            .await?;

        trace!(embed_response_text = %response_text);

        Ok::<String, Report>(response_text)
    })
    .await?
}

// Await the `JoinHandle` if the given `Option` is `Some(_)`
#[inline]
pub async fn maybe_join(maybe_spawned: Option<JoinHandle<Result<()>>>) -> Result<()> {
    if let Some(spawned) = maybe_spawned {
        return spawned.await?;
    }

    Ok(())
}
