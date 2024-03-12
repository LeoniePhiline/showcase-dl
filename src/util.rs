use std::{fmt::Debug, time::Duration};

use color_eyre::{
    eyre::{eyre, Result},
    Report,
};
use once_cell::sync::OnceCell;
use reqwest::{
    header::{HeaderMap, AUTHORIZATION, REFERER, RETRY_AFTER},
    Client, IntoUrl, Response, StatusCode, Url,
};
use tokio::task::JoinHandle;
use tracing::{info, instrument, trace, warn};

static CLIENT: OnceCell<Client> = OnceCell::new();

// Fetch a URL, applying a referer header
#[instrument]
pub(crate) async fn fetch_with_retry<U: IntoUrl + Debug>(
    url: U,
    maybe_referer: Option<&str>,
    maybe_authorization: Option<&str>,
) -> Result<Response> {
    let client = CLIENT.get_or_try_init(|| {
        Client::builder()
            .user_agent("Mozilla/5.0 (X11; U; Linux x86_64; en-US; rv:115.0esr) Gecko/20110619 Firefox/115.0esr")
            // Store cookies, as required to receive a JWT.
            // See `crate::process::event::get_jwt`.
            .cookie_store(true)
            .build()
    })?;

    let url = url.into_url()?;

    let request_headers = {
        let mut header_map = HeaderMap::new();

        if let Some(referer) = maybe_referer
            .map(TryInto::try_into)
            .transpose()
            .map_err(|_| eyre!("invalid `Referer` header value"))?
        {
            header_map.insert(REFERER, referer);
        }

        if let Some(authorization_header_value) = maybe_authorization
            .map(TryInto::try_into)
            .transpose()
            .map_err(|_| eyre!("invalid `Authorization` header value"))?
        {
            header_map.insert(AUTHORIZATION, authorization_header_value);
        }

        header_map
    };

    spawn_fetch_with_retry(client.clone(), url, request_headers).await
}

#[instrument]
async fn spawn_fetch_with_retry(
    client: Client,
    url: Url,
    request_headers: HeaderMap,
) -> Result<Response> {
    tokio::spawn(async move {
        let mut retries_remaining: u8 = 5;
        loop {
            let response = client
                .get(url.clone())
                .headers(request_headers.clone())
                .send()
                .await?;
            let response_headers = response.headers();
            trace!(?response_headers);

            // Wait and retry if rate-limited.
            let status_code = response.status();
            trace!(response.status = %status_code);
            if status_code == StatusCode::TOO_MANY_REQUESTS {
                // Try extracting number of seconds from `Retry-After` response header.
                // This header might also contain a date, but there is currently no need to support that.
                let wait_seconds = match response.headers().get(RETRY_AFTER) {
                    Some(header_value) => {
                        Ok::<Option<u64>, Report>(Some(header_value.to_str()?.parse()?))
                    }
                    None => Ok(None),
                }?
                .unwrap_or(60);

                if retries_remaining == 0 {
                    break Err(eyre!("rate limited throughout all retries"));
                }

                // Wait, then retry.
                warn!(%url, wait_seconds, "Received rate-limiting response. Waiting for retry. ({retries_remaining} retries remaining)");
                tokio::time::sleep(Duration::from_secs(wait_seconds)).await;

                retries_remaining -= 1;

                info!(%url, wait_seconds, "Retrying now. ({retries_remaining} further retries remaining)");
                continue;
            }

            return Ok::<Response, Report>(response);
        }
    })
    .await?
}

// Await the `JoinHandle` if the given `Option` is `Some(_)`
#[inline]
pub(crate) async fn maybe_join(maybe_spawned: Option<JoinHandle<Result<()>>>) -> Result<()> {
    if let Some(spawned) = maybe_spawned {
        return spawned.await?;
    }

    Ok(())
}
