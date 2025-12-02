use std::{fmt::Debug, sync::Arc};

use color_eyre::eyre::{eyre, Result};
use futures::future::join_all;
use tokio::sync::{oneshot, RwLock, RwLockReadGuard};
use tracing::{debug, info, instrument};

use self::video::Video;

pub(crate) mod video;

pub(crate) struct State {
    pub(crate) downloader: String,
    pub(crate) downloader_options: Vec<String>,

    stage: RwLock<Stage>,
    videos: RwLock<Vec<Arc<Video>>>,
}

pub(crate) enum Stage {
    Initializing,
    FetchingSource(String),
    Processing,
    // TODO: Semantic detail: Rename to `Finished` or keep at `Done`?
    Done,
    ShuttingDown,
}

impl State {
    pub(crate) fn new(downloader: String, downloader_options: Vec<String>) -> Self {
        Self {
            downloader,
            downloader_options,

            stage: RwLock::new(Stage::Initializing),
            videos: RwLock::new(vec![]),
        }
    }

    #[instrument(skip(self))]
    pub(crate) async fn set_stage_fetching_source(&self, page_url: impl Into<String> + Debug) {
        *self.stage.write().await = Stage::FetchingSource(page_url.into());
    }

    #[instrument(skip(self))]
    pub(crate) async fn set_stage_processing(&self) {
        *self.stage.write().await = Stage::Processing;
    }

    #[instrument(skip(self))]
    pub(crate) async fn set_stage_done(&self) {
        *self.stage.write().await = Stage::Done;
    }

    pub(crate) async fn stage(&self) -> RwLockReadGuard<'_, Stage> {
        self.stage.read().await
    }

    #[instrument(skip(self))]
    pub(crate) async fn push_video(&self, video: Arc<Video>) {
        let mut videos = self.videos.write().await;
        (*videos).push(video);
    }

    pub(crate) async fn videos(&self) -> RwLockReadGuard<'_, Vec<Arc<Video>>> {
        self.videos.read().await
    }

    #[instrument(skip(self))]
    pub(crate) async fn initiate_shutdown(
        &self,
        global_shutdown_complete: oneshot::Sender<()>,
    ) -> Result<()> {
        info!("Initiating shutdown.");

        // Set flag to refuse accepting new downloads (spawning new children).
        *self.stage.write().await = Stage::ShuttingDown;

        let mut children_shutdown = Vec::new();

        // Send SIGINT to all existing children.
        //
        // This causes the downloader to initiate clean shutdown
        // by muxing partially downloaded video and audio streams.
        let videos = self.videos().await;

        debug!("Sending SIGINT to child processes.");
        for video in &(*videos) {
            // Take each running download's single-use shutdown signal.
            //
            // We will await all currently running downloads
            // signaling their child process' graceful shutdown.
            if let Some(shutdown_signal) = (*video).take_shutdown_signal().await {
                children_shutdown.push(shutdown_signal);
            }

            (*video).initiate_shutdown().await?;
        }
        drop(videos);

        // Wait until all children have terminated.
        debug!(
            "Awaiting {} child processes shutting down.",
            children_shutdown.len()
        );
        join_all(children_shutdown).await;

        // Send shutdown-complete signal back to the UI's render loop.
        global_shutdown_complete
            .send(())
            .map_err(|()| eyre!("failed sending shutdown-complete signal"))?;

        Ok(())
    }

    pub(crate) async fn is_shutting_down(&self) -> bool {
        matches!(*self.stage.read().await, Stage::ShuttingDown)
    }
}
