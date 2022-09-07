use std::{fmt::Display, ops::Range, process::Stdio, sync::Arc};

use color_eyre::{
    eyre::{bail, eyre, ContextCompat, Result, WrapErr},
    Report,
};

use futures::{stream, TryStreamExt};
use json_dotpath::DotPaths;
use regex::Regex;
use reqwest::{header::HeaderValue, Client, Url};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;
use tracing::{debug, info};

mod args;
mod trace;

struct State {
    videos: RwLock<Vec<Arc<Video>>>,
}

struct Video {
    url: String,
    title: RwLock<Option<String>>,
    line: RwLock<Option<String>>,
}

enum Progress {
    Raw(String),
    Parsed {
        line: String,
        percent: Option<f32>,
        size: Option<Range<usize>>,
        speed: Option<Range<usize>>,
        eta: Option<Range<usize>>,
        frag: Option<u16>,
        frag_total: Option<u16>,
    },
}

impl State {
    fn new() -> Self {
        Self {
            videos: RwLock::new(vec![]),
        }
    }

    async fn push_video(&self, video: Arc<Video>) {
        let mut videos = self.videos.write().await;
        (*videos).push(video);
    }
}

impl Video {
    fn new(url: String) -> Self {
        Self::new_with_title(url, None)
    }

    fn new_with_title(url: String, title: Option<String>) -> Self {
        Self {
            url,
            title: RwLock::new(title),
            line: RwLock::new(None),
        }
    }

    async fn progress(&self) -> Result<Option<Progress>> {
        // TODO: Use lazy_static to compile Regex only once!
        let re = Regex::new(
            r#"\[download\]\s+(?P<percent>[\d+\.]+?)% of (?P<size>~?[\d+\.]+?(?:[KMG]i)B) at\s+(?P<speed>(?:~?[\d+\.]+?(?:[KMG]i)?|Unknown )B/s) ETA\s+(?P<eta>(?:[\d:-]+|Unknown))(?: \(frag (?P<frag>\d+)/(?P<frag_total>\d+)\))?"#,
        )?;
        // TODO: "Finished" looks like this: '[download] 100% of 956.44MiB in 00:15 at 63.00MiB/s' - maybe no need to parse; can show as `Progress::Raw(line)`.

        let maybe_line = self.line.read().await;
        Ok(match *maybe_line {
            Some(ref line) => {
                let line = line.clone();
                let maybe_captures = re.captures(&line);
                match maybe_captures {
                    Some(captures) => {
                        let percent = captures
                            .name("percent")
                            .and_then(|percent_match| percent_match.as_str().parse::<f32>().ok());

                        let size = captures.name("size").map(|size_match| size_match.range());
                        let speed = captures
                            .name("speed")
                            .map(|speed_match| speed_match.range());
                        let eta = captures.name("eta").map(|eta_match| eta_match.range());

                        let frag = captures
                            .name("frag")
                            .and_then(|frag_match| frag_match.as_str().parse::<u16>().ok());

                        let frag_total = captures.name("frag_total").and_then(|frag_total_match| {
                            frag_total_match.as_str().parse::<u16>().ok()
                        });
                        Some(Progress::Parsed {
                            line,
                            percent,
                            size,
                            speed,
                            eta,
                            frag,
                            frag_total,
                        })
                    }
                    None => Some(Progress::Raw(line)),
                }
            }
            None => None,
        })
    }
}

impl Display for Progress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parsed {
                line,
                percent,
                size,
                speed,
                eta,
                frag,
                frag_total,
            } => {
                write!(f, "Parsed progress: ")?;
                if let Some(percent) = percent {
                    write!(f, "{:.1} % done. ", percent)?;
                }
                if let Some(size) = &size {
                    write!(f, "file size: {}. ", &line[size.clone()])?;
                }
                if let Some(speed) = &speed {
                    write!(f, "download speed: {}. ", &line[speed.clone()])?;
                }
                if let Some(eta) = &eta {
                    write!(f, "ETA: {}. ", &line[eta.clone()])?;
                }
                if let Some(frag) = frag {
                    write!(f, "fragments: {}", frag)?;
                    if let Some(frag_total) = frag_total {
                        write!(f, " / {}", frag_total)?;
                    }
                    write!(f, ". ")?;
                }
            }
            Progress::Raw(line) => write!(f, "Raw progress: {line}")?,
        }

        Ok(())
    }
}

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
    // TODO: Use lazy_static to compile Regex only once!
    let re =
        Regex::new(r#"<iframe[^>]+ src="(?P<embed_url>https://player\.vimeo\.com/video/[^"]+)""#)?;

    stream::iter(re.captures_iter(page_body).map(Ok))
        .try_for_each_concurrent(None, |captures| {
            let referer = &referer;
            let state = state.clone();
            async move {
                debug!("{captures:#?}");

                match captures.name("embed_url") {
                    Some(embed_url_match) => {
                        let embed_url = html_escape::decode_html_entities(embed_url_match.as_str())
                            .into_owned();

                        let video = Arc::new(Video::new(embed_url));
                        (*state).push_video(video.clone()).await;

                        tokio::try_join!(
                            {
                                let video = video.clone();
                                async move {
                                    debug!("Fetch title for simple embed '{}'...", &video.url);
                                    extract_simple_embed_title(video, referer).await?;
                                    Ok::<(), Report>(())
                                }
                            },
                            async move {
                                info!("Download simple embed '{}'...", &video.url);
                                download_video(video, referer).await?;
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
    let response_text = fetch_with_referer(&video.url, referer).await?;

    // TODO: Use lazy_static to compile Regex only once.
    let maybe_captures = Regex::new(r#"<title>(?P<title>.*?)</title>"#)?.captures(&response_text);

    if let Some(captures) = maybe_captures {
        if let Some(title_match) = captures.name("title") {
            let matched_title = title_match.as_str();
            debug!(
                "Matched title '{matched_title}' for simple embed '{}'",
                &video.url
            );
            let mut title = video.title.write().await;
            *title = Some(matched_title.into());
        }
    }

    Ok(())
}

async fn process_showcases(page_body: &str, referer: &str, state: Arc<State>) -> Result<()> {
    // TODO: Use lazy_static to only compile once.
    let re = Regex::new(r#"<iframe[^>]+ src="(?P<embed_url>https://vimeo\.com/showcase/[^"]+)""#)?;

    stream::iter(re.captures_iter(page_body).map(Ok))
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
    let response_text = fetch_with_referer(embed_url, referer).await?;

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

    let maybe_title = config.dot_get::<String>("video.title")?;

    debug!("config embed_code: {embed_code:#?}");
    // TODO: Use lazy_static to not re-compile this regex in a loop? https://docs.rs/regex/latest/regex/#example-avoid-compiling-the-same-regex-in-a-loop
    let re = Regex::new(r#"src="(?P<embed_url>[^"]+)""#)?;
    let captures = re.captures(&embed_code).ok_or_else(|| {
        eyre!(
            "Could not extract embed URL from config 'video.embed_code' string (no regex captures)"
        )
    })?;

    match captures.name("embed_url") {
        Some(embed_url_match) => {
            debug!("embed_url_match: {embed_url_match:#?}");

            let embed_url =
                html_escape::decode_html_entities(embed_url_match.as_str()).into_owned();
            info!("Download showcase clip '{embed_url}'...");

            let video = Arc::new(Video::new_with_title(embed_url, maybe_title));
            (*state).push_video(video.clone()).await;
            download_video(video, referer).await?;
        }
        None => {
            bail!("Could not extract embed URL from config 'video.embed_code' string (embed_url not captured)");
        }
    }

    Ok(())
}

async fn download_video(video: Arc<Video>, referer: &str) -> Result<()> {
    debug!(
        "Spawn: yt-dlp --newline --no-colors --referer '{}' '{}'",
        referer, &video.url
    );
    let mut child = tokio::process::Command::new("yt-dlp")
        .kill_on_drop(true)
        .arg("--newline")
        .arg("--no-colors")
        .stdout(Stdio::piped())
        .arg("--referer")
        .arg(referer)
        .arg(&video.url)
        .spawn()
        .wrap_err("yt-dlp command failed to start")?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| eyre!("Child's stdout was None"))?;
    let mut lines = BufReader::new(stdout).lines();
    tokio::spawn(async move {
        while let Some(next_line) = lines.next_line().await? {
            {
                let title = video.title.read().await;
                debug!(
                    "Line from '{}': '{next_line}'",
                    match *title {
                        Some(ref title) => title,
                        None => &video.url,
                    }
                );
            }
            {
                let mut line = video.line.write().await;
                *line = Some(next_line);
            }

            match video.progress().await? {
                Some(progress) => info!("{progress}"),
                None => info!("No progress"),
            };
        }

        Ok::<(), Report>(())
    })
    .await??; // TODO: Join with child.wait(), or just run .wait() after?

    child
        .wait()
        .await
        .wrap_err("yt-dlp command failed to run")?;

    Ok(())
}

async fn fetch_with_referer(url: &str, referer: &str) -> Result<String> {
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
