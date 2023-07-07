use std::{path::Path, process::Stdio, sync::Arc};

use color_eyre::{
    eyre::{Result, WrapErr},
    Report,
};
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::{Child, Command},
    sync::{RwLock, RwLockReadGuard},
    task::JoinHandle,
};
use tracing::{debug, error, trace};

use crate::util::maybe_join;
use progress::ProgressDetail;

pub mod progress;

pub struct Video {
    stage: RwLock<Stage>,
    url: String,
    referer: String,
    title: RwLock<Option<String>>,
    line: RwLock<Option<String>>,
    output_file: RwLock<Option<String>>,
    percent_done: RwLock<Option<f64>>,
}
pub enum Stage {
    Initializing,
    Downloading,
    ExtractingAudio,
    Finished,
}

pub struct VideoRead<'a> {
    stage: RwLockReadGuard<'a, Stage>,
    url: &'a str,
    title: RwLockReadGuard<'a, Option<String>>,
    line: RwLockReadGuard<'a, Option<String>>,
    output_file: RwLockReadGuard<'a, Option<String>>,
    percent_done: RwLockReadGuard<'a, Option<f64>>,
}

static RE_OUTPUT_FILE_DOWNLOADING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\[download\] Destination: (?P<output_file>.+)$"#).unwrap());

static RE_OUTPUT_FILE_ALREADY_DOWNLOADED: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^\[download\] (?P<output_file>.+?) has already been downloaded$"#).unwrap()
});

static RE_OUTPUT_FILE_MERGING: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^\[Merger\] Merging formats into "(?P<output_file>.+?)"$"#).unwrap()
});

static RE_PERCENT_DONE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\[download\]\s+(?P<percent_done>[\d+\.]+?)%"#).unwrap());

static REGEX_DOWNLOAD_PROGRESS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^\[download\]\s+(?P<percent>[\d+\.]+?)% of\s+(?P<size>(?:~\s*)?[\d+\.]+?(?:[KMG]i)B)(?: at\s+(?P<speed>(?:(?:~\s*)?[\d+\.]+?(?:[KMG]i)?|Unknown )B/s))?(?: ETA\s+(?P<eta>(?:[\d:-]+|Unknown)))?(?: \(frag (?P<frag>\d+)/(?P<frag_total>\d+)\))?"#).unwrap()
});

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
            output_file: RwLock::new(None),
            percent_done: RwLock::new(None),
        }
    }

    pub async fn set_stage_downloading(&self) {
        *self.stage.write().await = Stage::Downloading;
    }

    pub async fn set_stage_extracting_audio(&self) {
        *self.stage.write().await = Stage::ExtractingAudio;
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
        f(&title)
    }

    pub async fn update_title(&self, new_title: String) {
        let mut title = self.title.write().await;
        *title = Some(new_title);
    }

    pub async fn title(&self) -> RwLockReadGuard<Option<String>> {
        self.title.read().await
    }

    pub async fn update_line(&self, new_line: String) {
        self.extract_output_file(&new_line).await;
        self.extract_percent_done(&new_line).await;

        // Store the line to ref to it for size, speed and ETA ranges.
        let mut line = self.line.write().await;
        *line = Some(new_line);
    }

    async fn extract_output_file(&self, line: &str) {
        // Extract output file if present in the current line
        let maybe_captures = RE_OUTPUT_FILE_DOWNLOADING
            .captures(line)
            .or_else(|| RE_OUTPUT_FILE_ALREADY_DOWNLOADED.captures(line))
            .or_else(|| RE_OUTPUT_FILE_MERGING.captures(line));
        if let Some(captures) = maybe_captures {
            if let Some(output_file) = captures
                .name("output_file")
                .map(|output_file_match| output_file_match.as_str().into())
            {
                self.update_output_file(output_file).await;
            }
        }
    }

    async fn extract_percent_done(&self, line: &str) {
        // Extract current percent done if present in the current line
        let maybe_captures = RE_PERCENT_DONE.captures(line);
        if let Some(captures) = maybe_captures {
            if let Some(percent_done) = captures
                .name("percent_done")
                .and_then(|percent_done_match| percent_done_match.as_str().parse::<f64>().ok())
            {
                self.update_percent_done(percent_done).await;
            }
        }
    }

    pub async fn line(&self) -> RwLockReadGuard<Option<String>> {
        self.line.read().await
    }

    pub async fn update_percent_done(&self, new_percent: f64) {
        let mut percent_done = self.percent_done.write().await;
        *percent_done = Some(new_percent);
    }

    pub async fn percent_done(&self) -> RwLockReadGuard<Option<f64>> {
        self.percent_done.read().await
    }

    pub async fn update_output_file(&self, new_output_file: String) {
        let mut output_file = self.output_file.write().await;
        *output_file = Some(new_output_file);
    }

    pub async fn output_file(&self) -> RwLockReadGuard<Option<String>> {
        self.output_file.read().await
    }

    pub async fn download(self: Arc<Self>, bin: &str) -> Result<()> {
        self.set_stage_downloading().await;

        let cmd = format!(
            "{} --newline --no-colors --add-header 'Referer:{}' '{}'",
            bin,
            &self.referer,
            self.url()
        );

        debug!("Spawn: {cmd}");
        self.clone()
            .child_read_to_end(
                Command::new(bin)
                    .kill_on_drop(true)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .arg("--newline")
                    .arg("--no-colors")
                    .arg("--add-header")
                    .arg(format!("Referer:{}", self.referer))
                    .arg(self.url())
                    .spawn()
                    .wrap_err_with(|| format!("Command failed to start: {cmd}"))?,
            )
            .await?;

        self.set_stage_finished().await;

        Ok(())
    }

    pub async fn extract_audio(self: Arc<Self>, format: &str) -> Result<()> {
        if let Some(ref output_file) = *self.output_file().await {
            self.set_stage_extracting_audio().await;

            let source = Path::new(output_file);
            let destination = Path::new(output_file).with_extension(format);

            let cmd = format!(
                "ffmpeg -y -i '{}' '{}'",
                source.to_string_lossy(),
                destination.to_string_lossy()
            );

            debug!("Spawn: {cmd}");
            self.clone()
                // TODO: Need a different read strategy. `-progress pipe:1` gives multi-line progress reports each second.
                //       These need to be parsed or appended somehow to form a line.
                //       Alternatively, if we work without
                .child_read_to_end(
                    Command::new("ffmpeg")
                        .kill_on_drop(true)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .arg("-nostdin")
                        // TODO: Make audio extraction overwriting of existing files depend on argument
                        .arg("-y")
                        .arg("-i")
                        .arg(source)
                        .arg(&destination)
                        .spawn()
                        .wrap_err_with(|| format!("Command failed to start: {cmd}"))?,
                )
                .await?;

            self.set_stage_finished().await;
        }

        Ok(())
    }

    async fn child_read_to_end(self: Arc<Self>, mut child: Child) -> Result<()> {
        let consume_stdout = child
            .stdout
            .take()
            .map(|stdout| self.clone().consume_stream(stdout));

        let consume_stderr = child
            .stderr
            .take()
            .map(|stderr| self.clone().consume_stream(stderr));

        let await_exit = async {
            tokio::spawn(async move {
                child
                    .wait()
                    .await
                    .wrap_err("yt-dlp command failed to run")?;

                Ok::<(), Report>(())
            })
            .await??;

            Ok(())
        };

        tokio::try_join!(
            maybe_join(consume_stdout),
            maybe_join(consume_stderr),
            await_exit,
        )
        .wrap_err("Could not join child consumers for stdout, stderr and awaiting child exit.")?;

        Ok(())
    }

    fn consume_stream<A: AsyncRead + Unpin + Send + 'static>(
        self: Arc<Self>,
        reader: A,
    ) -> JoinHandle<Result<()>> {
        let mut lines = BufReader::new(reader).lines();

        let video = self;
        tokio::spawn(async move {
            while let Some(next_line) = lines.next_line().await? {
                video
                    .use_title(|title| {
                        let title = match *title {
                            Some(ref title) => title,
                            None => video.url(),
                        };
                        if next_line.starts_with("ERROR:") {
                            error!("Line from '{title}': '{next_line}'",)
                        } else {
                            trace!("Line from '{title}': '{next_line}'",)
                        }
                    })
                    .await;

                video.update_line(next_line).await;
            }

            Ok::<(), Report>(())
        })
    }

    // Acquire read guards for all fine-grained access-controlled fields.
    pub async fn read(&self) -> VideoRead {
        VideoRead {
            stage: self.stage().await,
            url: &self.url,
            title: self.title().await,
            line: self.line().await,
            output_file: self.output_file().await,
            percent_done: self.percent_done().await,
        }
    }
}

impl<'a> VideoRead<'a> {
    pub fn stage(&self) -> &Stage {
        &self.stage
    }

    pub fn url(&self) -> &'a str {
        self.url
    }

    pub fn title(&self) -> &Option<String> {
        &self.title
    }

    pub fn progress_detail(&'a self) -> Option<ProgressDetail<'a>> {
        match *self.line {
            Some(ref line) => {
                let maybe_captures = REGEX_DOWNLOAD_PROGRESS.captures(line.as_str());
                match maybe_captures {
                    Some(captures) => {
                        let percent = captures
                            .name("percent")
                            .and_then(|percent_match| percent_match.as_str().parse::<f64>().ok())
                            // Fall back to last stored progress percentage if current line does not provide a fresh value.
                            .or(*self.percent_done);

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

    pub fn output_file(&self) -> &Option<String> {
        &self.output_file
    }

    pub fn percent_done(&self) -> &Option<f64> {
        &self.percent_done
    }
}
