use color_eyre::eyre::{bail, ContextCompat, Result};

use futures::{stream, TryStreamExt};
use regex::Regex;
use reqwest::{header::HeaderValue, Client, Url};
use serde_json::Value;
use tracing::{debug, info};
use tracing_attributes::instrument;

mod args;
mod trace;

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

    let response = Client::new().get(page_url).send().await?;
    debug!(page_response = ?response);

    let response_text = response.text().await?;
    debug!(page_response_text = ?response_text);

    tokio::try_join!(
        process_showcases(&response_text, &referer),
        process_simple_embeds(&response_text, &referer)
    )?;

    Ok(())
}

async fn process_simple_embeds(page_body: &str, referer: &str) -> Result<()> {
    let re =
        Regex::new(r#"<iframe[^>]+ src="(?P<embed_url>https://player\.vimeo\.com/video/[^"]+)""#)?;

    stream::iter(re.captures_iter(page_body).map(Ok))
        .try_for_each_concurrent(None, |captures| {
            let referer = &referer;
            async move {
                debug!("{captures:#?}");

                match captures.name("embed_url") {
                    Some(embed_url_match) => {
                        let embed_url = embed_url_match.as_str();
                        info!("Download simple embed '{embed_url}'...");
                        download_video(embed_url, referer).await
                    }
                    None => bail!("Capture group did not match named 'embed_url'"),
                }
            }
        })
        .await?;

    Ok(())
}

async fn process_showcases(page_body: &str, referer: &str) -> Result<()> {
    let re = Regex::new(r#"<iframe[^>]+ src="(?P<embed_url>https://vimeo\.com/showcase/[^"]+)""#)?;

    stream::iter(re.captures_iter(page_body).map(Ok))
        .try_for_each_concurrent(None, |captures| {
            let referer = &referer;
            async move {
                debug!("{captures:#?}");

                match captures.name("embed_url") {
                    Some(embed_url_match) => {
                        let embed_url = embed_url_match.as_str();
                        info!("Extract clips from showcase '{embed_url}'...");
                        process_showcase_embed(embed_url, referer).await
                    }
                    None => bail!("Capture group did not match named 'embed_url'"),
                }
            }
        })
        .await?;

    Ok(())
}

async fn process_showcase_embed(embed_url: &str, referer: &str) -> Result<()> {
    let mut referer_header_map = reqwest::header::HeaderMap::new();
    referer_header_map.insert(reqwest::header::REFERER, HeaderValue::from_str(referer)?);

    let response = Client::new()
        .get(embed_url)
        .headers(referer_header_map)
        .send()
        .await?;
    debug!(embed_response = ?response);

    let response_text = response.text().await?;
    debug!(embed_response_text = ?response_text);

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

    match data {
        Value::Object(map) => {
            match map.get("clips") {
                Some(Value::Array(clips)) => {
                    // TODO: Process in parallel. (`.for_each_concurrent()` with `clips.len()`)
                    stream::iter(clips.iter().map(Ok))
                        .try_for_each_concurrent(None, |clip| async move {
                            process_showcase_clip(clip, referer).await
                        })
                        .await?;
                }
                Some(Value::Null) => {
                    bail!("'app-data' contained a 'clips' key, but its value was `null`. Commonly, this means 'Access Denied' - check your referer!")
                }
                Some(_) => {
                    bail!("'app-data' contained a 'clips' key, but it was not an array, as was expected.")
                }
                None => {
                    bail!("'app-data' did not contain a 'clips' key!");
                }
            }
        }
        _ => {
            bail!("'app-data' was not a JSON object");
        }
    }

    Ok(())
}

async fn process_showcase_clip(clip: &Value, referer: &str) -> Result<()> {
    match clip {
        Value::Object(map) => {
            let config_url = map.get("config").wrap_err("Clip had no 'config' key")?;

            match config_url {
                Value::String(config_url) => {
                    let client = Client::new();
                    let response = client.get(config_url).send().await?;
                    debug!(config_response = ?response);

                    let response_text = response.text().await?;
                    debug!(config_response_text = ?response_text);

                    let data: Value = serde_json::from_str(&response_text)?;
                    debug!("config response data: {data:#?}");

                    match data {
                        Value::Object(map) => {
                            match map.get("video") {
                                Some(Value::Object(map)) => {
                                    match map.get("embed_code") {
                                        Some(Value::String(embed_code)) => {
                                            debug!("config embed_code: {embed_code:#?}");
                                            // TODO: Use lazy_static to not re-compile this regex in a loop? https://docs.rs/regex/latest/regex/#example-avoid-compiling-the-same-regex-in-a-loop
                                            let re = Regex::new(r#"src="(?P<embed_url>[^"]+)""#)?;
                                            let captures = re.captures(embed_code);

                                            match captures {
                                                Some(captures) => {
                                                    match captures.name("embed_url") {
                                                        Some(embed_url_match) => {
                                                            debug!("embed_url_match: {embed_url_match:#?}");

                                                            let embed_url =
                                                                embed_url_match.as_str();
                                                            info!("Download showcase clip '{embed_url}'...");

                                                            download_video(embed_url, referer)
                                                                .await?;
                                                        }
                                                        None => {
                                                            bail!("Could not extract embed URL from config 'video.embed_code' string (embed_url not captured)");
                                                        }
                                                    }
                                                }
                                                None => {
                                                    bail!("Could not extract embed URL from config 'video.embed_code' string (no captures)");
                                                }
                                            }
                                        }
                                        Some(_) => {
                                            bail!("Config response's 'video.embed_code' key was not a JSON string");
                                        }
                                        None => {
                                            bail!("Config response's 'video.embed_code' key did not exist");
                                        }
                                    }
                                }
                                Some(_) => {
                                    bail!("Config response's 'video' key  was not a JSON object");
                                }
                                None => {
                                    bail!("Config response's 'video' key did not exist");
                                }
                            }
                        }
                        _ => {
                            bail!("Config response was not a JSON object");
                        }
                    }
                }
                _ => bail!("Clip had a 'config' key, but it was not of a String type"),
            }

            Ok(())
        }
        _ => bail!("Clip was not an object"),
    }
}

async fn download_video(url: &str, referer: &str) -> Result<()> {
    info!("RUN: yt-dlp --referer '{}' '{}'", referer, url);
    tokio::process::Command::new("yt-dlp")
        .arg("--referer")
        .arg(referer)
        .arg(url)
        .spawn()
        .expect("yt-dlp command failed to start")
        .wait()
        .await
        .expect("yt-dlp command failed to run");

    Ok(())
}
