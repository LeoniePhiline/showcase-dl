use color_eyre::{
    eyre::{eyre, Result, WrapErr},
    Report,
};
use lazy_static::lazy_static;
use regex::Regex;
use std::{process::Stdio, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{RwLock, RwLockReadGuard},
};
use tracing::debug;

use self::progress::ProgressDetail;

pub mod progress;

pub struct Video {
    stage: RwLock<Stage>,
    url: String,
    referer: String,
    title: RwLock<Option<String>>,
    line: RwLock<Option<String>>,
    last_percent: RwLock<Option<f64>>,
}
pub enum Stage {
    Initializing,
    Downloading,
    Finished,
}

pub struct VideoRead<'a> {
    stage: RwLockReadGuard<'a, Stage>,
    url: &'a str,
    title: RwLockReadGuard<'a, Option<String>>,
    line: RwLockReadGuard<'a, Option<String>>,
    last_percent: RwLockReadGuard<'a, Option<f64>>,
}

impl Video {
    pub fn new(url: impl Into<String>, referer: impl Into<String>) -> Self {
        Self::new_with_title(url.into(), referer.into(), None)
    }

    pub fn new_with_title(
        url: impl Into<String>,
        referer: impl Into<String>,
        title: Option<String>,
    ) -> Self {
        Self {
            stage: RwLock::new(Stage::Initializing),
            url: url.into(),
            referer: referer.into(),
            title: RwLock::new(title),
            line: RwLock::new(None),
            last_percent: RwLock::new(None),
        }
    }

    pub async fn set_stage_downloading(&self) {
        *self.stage.write().await = Stage::Downloading;
    }

    pub async fn set_stage_finished(&self) {
        *self.stage.write().await = Stage::Finished;
    }

    pub async fn stage(&self) -> RwLockReadGuard<Stage> {
        self.stage.read().await
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn use_title<F, O>(&self, f: F) -> O
    where
        F: FnOnce(&Option<String>) -> O,
    {
        let title = self.title.read().await;
        f(&*title)
    }

    pub async fn update_title(&self, new_title: String) {
        let mut title = self.title.write().await;
        *title = Some(new_title);
    }

    pub async fn title(&self) -> RwLockReadGuard<Option<String>> {
        self.title.read().await
    }

    pub async fn update_line(&self, new_line: String) {
        lazy_static! {
            static ref RE: Regex =
                Regex::new(r#"^\[download\]\s+(?P<percent>[\d+\.]+?)%"#,).unwrap();
        }

        // Extract current percent if present in the current line
        let maybe_captures = RE.captures(&new_line);
        if let Some(captures) = maybe_captures {
            if let Some(percent) = captures
                .name("percent")
                .and_then(|percent_match| percent_match.as_str().parse::<f64>().ok())
            {
                self.update_last_percent(percent).await;
            }
        }

        // Store the line to ref to it for size, speed and ETA ranges.
        let mut line = self.line.write().await;
        *line = Some(new_line);
    }

    pub async fn line(&self) -> RwLockReadGuard<Option<String>> {
        self.line.read().await
    }

    pub async fn update_last_percent(&self, new_percent: f64) {
        let mut last_percent = self.last_percent.write().await;
        *last_percent = Some(new_percent);
    }

    pub async fn last_percent(&self) -> RwLockReadGuard<Option<f64>> {
        self.last_percent.read().await
    }

    pub async fn download(self: Arc<Self>) -> Result<()> {
        self.set_stage_downloading().await;

        debug!(
            "Spawn: yt-dlp --newline --no-colors --referer '{}' '{}'",
            &self.referer,
            self.url()
        );
        let mut child = tokio::process::Command::new("yt-dlp")
            .kill_on_drop(true)
            .arg("--newline")
            .arg("--no-colors")
            .stdout(Stdio::piped())
            .arg("--referer")
            .arg(&self.referer)
            .arg(self.url())
            .spawn()
            .wrap_err("yt-dlp command failed to start")?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| eyre!("Child's stdout was None"))?;

        let mut lines = BufReader::new(stdout).lines();

        let video = self.clone();
        let process_pipe = tokio::spawn(async move {
            while let Some(next_line) = lines.next_line().await? {
                video
                    .use_title(|title| {
                        debug!(
                            "Line from '{}': '{next_line}'",
                            match *title {
                                Some(ref title) => title,
                                None => video.url(),
                            }
                        )
                    })
                    .await;

                video.update_line(next_line).await;
            }

            Ok::<(), Report>(())
        });

        let process_wait = tokio::spawn(async move {
            child
                .wait()
                .await
                .wrap_err("yt-dlp command failed to run")?;

            Ok::<(), Report>(())
        });

        tokio::try_join!(async { process_pipe.await? }, async { process_wait.await? },)?;

        self.set_stage_finished().await;

        Ok(())
    }

    // Acquire read guards for all fine-grained access-controlled fields.
    pub async fn read(&self) -> VideoRead {
        VideoRead {
            stage: self.stage().await,
            url: &self.url,
            title: self.title().await,
            line: self.line().await,
            last_percent: self.last_percent().await,
        }
    }
}

impl<'a> VideoRead<'a> {
    pub fn stage(&self) -> &Stage {
        &(*self.stage)
    }

    pub fn url(&self) -> &'a str {
        self.url
    }

    pub fn title(&self) -> &Option<String> {
        &(*self.title)
    }

    pub fn progress_detail(&'a self) -> Option<ProgressDetail<'a>> {
        lazy_static! {
            static ref RE: Regex = Regex::new(
                r#"^\[download\]\s+(?P<percent>[\d+\.]+?)% of (?P<size>~?[\d+\.]+?(?:[KMG]i)B)(?: at\s+(?P<speed>(?:~?[\d+\.]+?(?:[KMG]i)?|Unknown )B/s))?(?: ETA\s+(?P<eta>(?:[\d:-]+|Unknown)))?(?: \(frag (?P<frag>\d+)/(?P<frag_total>\d+)\))?"#,
            ).unwrap();
        }

        match *self.line {
            Some(ref line) => {
                let maybe_captures = RE.captures(line.as_str());
                match maybe_captures {
                    Some(captures) => {
                        let percent = captures
                            .name("percent")
                            .and_then(|percent_match| percent_match.as_str().parse::<f64>().ok())
                            // Fall back to last stored progress percentage if current line does not provide a fresh value.
                            .or(*self.last_percent);

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
                        Some(ProgressDetail::Parsed {
                            line,
                            percent,
                            size,
                            speed,
                            eta,
                            frag,
                            frag_total,
                        })
                    }
                    None => Some(ProgressDetail::Raw(line)),
                }
            }
            None => None,
        }
    }

    pub fn last_percent(&self) -> &Option<f64> {
        &(*self.last_percent)
    }
}
