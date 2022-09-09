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

use self::progress::VideoProgress;

pub mod progress;

// TODO: Might `Cow` be of any help here? Can I use references for any of this?
pub struct Video {
    url: String,
    referer: String,
    title: RwLock<Option<String>>,
    line: RwLock<Option<String>>,
}

pub struct VideoRead<'a> {
    url: &'a str,
    title: RwLockReadGuard<'a, Option<String>>,
    line: RwLockReadGuard<'a, Option<String>>,
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
            url: url.into(),
            referer: referer.into(),
            title: RwLock::new(title),
            line: RwLock::new(None),
        }
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
        let mut line = self.line.write().await;
        *line = Some(new_line);
    }

    pub async fn line(&self) -> RwLockReadGuard<Option<String>> {
        self.line.read().await
    }

    pub async fn download(self: Arc<Self>) -> Result<()> {
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

        // TODO: Distinguish video progress/state between living child process (downloading / processing) and ended child process ("Done!")

        let process_pipe = tokio::spawn(async move {
            while let Some(next_line) = lines.next_line().await? {
                self.use_title(|title| {
                    debug!(
                        "Line from '{}': '{next_line}'",
                        match *title {
                            Some(ref title) => title,
                            None => self.url(),
                        }
                    )
                })
                .await;

                self.update_line(next_line).await;
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

        // TODO: Distinguish video progress/state between living child process (downloading / processing) and ended child process ("Done!")

        Ok(())
    }

    // Acquire read guards for all fine-grained access-controlled fields.
    pub async fn read(&self) -> VideoRead {
        VideoRead {
            url: &self.url,
            title: self.title().await,
            line: self.line().await,
        }
    }
}

impl<'a> VideoRead<'a> {
    pub fn url(&self) -> &'a str {
        self.url
    }

    pub fn title(&self) -> &Option<String> {
        &(*self.title)
    }

    // TODO: Should this be a method on the `VideoRead` struct impl instead of the `Video` struct impl?
    //       We need to already have the line acquired anyway.
    pub fn progress(&'a self) -> Option<VideoProgress<'a>> {
        lazy_static! {
            static ref RE: Regex = Regex::new(
                // TODO: "Finished" looks like this: '[download] 100% of 956.44MiB in 00:15 at 63.00MiB/s'.
                //       This is currently not parsed. Maybe there is no need to parse? -> Can show as `Progress::Raw(line)`.
                r#"\[download\]\s+(?P<percent>[\d+\.]+?)% of (?P<size>~?[\d+\.]+?(?:[KMG]i)B)(?: at\s+(?P<speed>(?:~?[\d+\.]+?(?:[KMG]i)?|Unknown )B/s))?(?: ETA\s+(?P<eta>(?:[\d:-]+|Unknown)))?(?: \(frag (?P<frag>\d+)/(?P<frag_total>\d+)\))?"#,
            ).unwrap();
        }

        match *self.line {
            Some(ref line) => {
                let maybe_captures = RE.captures(line.as_str());
                match maybe_captures {
                    Some(captures) => {
                        let percent = captures
                            .name("percent")
                            .and_then(|percent_match| percent_match.as_str().parse::<f64>().ok());

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
                        Some(VideoProgress::Parsed {
                            line,
                            percent,
                            size,
                            speed,
                            eta,
                            frag,
                            frag_total,
                        })
                    }
                    None => Some(VideoProgress::Raw(line)),
                }
            }
            None => None,
        }
    }
}
