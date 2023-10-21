use std::{sync::Arc, time::Duration};

use color_eyre::eyre::{eyre, Result};
use futures::{stream, StreamExt};
use tokio::sync::{oneshot, RwLock, RwLockReadGuard};
use tracing::{debug, info};

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

    pub(crate) async fn set_stage_fetching_source(&self, page_url: impl Into<String>) {
        *self.stage.write().await = Stage::FetchingSource(page_url.into());
    }

    pub(crate) async fn set_stage_processing(&self) {
        *self.stage.write().await = Stage::Processing;
    }

    pub(crate) async fn set_stage_done(&self) {
        *self.stage.write().await = Stage::Done;
    }

    pub(crate) async fn stage(&self) -> RwLockReadGuard<Stage> {
        self.stage.read().await
    }

    pub(crate) async fn push_video(&self, video: Arc<Video>) {
        let mut videos = self.videos.write().await;
        (*videos).push(video);
    }

    pub(crate) async fn videos(&self) -> RwLockReadGuard<Vec<Arc<Video>>> {
        self.videos.read().await
    }

    pub(crate) async fn initiate_shutdown(
        &self,
        tx_shutdown_complete: oneshot::Sender<()>,
    ) -> Result<()> {
        info!("Initiating shutdown.");

        // Set flag to refuse accepting new downloads (spawning new children).
        *self.stage.write().await = Stage::ShuttingDown;

        debug!("Sending SIGINT to child processes.");

        // Send SIGINT to all existing children.
        //
        // This causes the downloader to initiate clean shutdown
        // by muxing partially downloaded video and audio streams.
        let videos = self.videos().await;
        for video in (*videos).iter() {
            (*video).initiate_shutdown().await?;
        }
        drop(videos);

        // Then check every 50ms if all children have terminated.
        while !self.all_children_terminated().await {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Send shutdown-complete signal back to the UI's render loop.
        tx_shutdown_complete
            .send(())
            .map_err(|_| eyre!("failed sending shutdown-complete signal"))?;

        Ok(())
    }

    async fn all_children_terminated(&self) -> bool {
        let videos = self.videos().await;

        // Determine if all children have either finished or failed.
        stream::iter((*videos).iter())
            .all(|video| async {
                // Is any download *not* yet finished or failed?
                // Then we will send an interrupt signal
                // and wait for a few seconds before retrying.
                matches!(
                    *video.stage().await,
                    self::video::Stage::Finished | self::video::Stage::Failed
                )
            })
            .await
    }

    pub(crate) async fn is_shutting_down(&self) -> bool {
        matches!(*self.stage.read().await, Stage::ShuttingDown)
    }
}
