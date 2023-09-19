use std::{sync::Arc, time::Duration};

use color_eyre::eyre::Result;
use futures::{stream, StreamExt};
use tokio::sync::{RwLock, RwLockReadGuard};
use tracing::{debug, info};

use self::video::Video;

pub mod video;

pub struct State {
    pub downloader: String,
    pub downloader_options: Vec<String>,

    stage: RwLock<Stage>,
    videos: RwLock<Vec<Arc<Video>>>,
}

pub enum Stage {
    Initializing,
    FetchingSource(String),
    Processing,
    // TODO: Semantic detail: Rename to `Finished` or keep at `Done`?
    Done,
    ShuttingDown,
}

impl State {
    pub fn new(downloader: String, downloader_options: Vec<String>) -> Self {
        Self {
            downloader,
            downloader_options,

            stage: RwLock::new(Stage::Initializing),
            videos: RwLock::new(vec![]),
        }
    }

    pub async fn set_stage_fetching_source<'a>(&self, page_url: impl Into<String>) {
        *self.stage.write().await = Stage::FetchingSource(page_url.into());
    }

    pub async fn set_stage_processing(&self) {
        *self.stage.write().await = Stage::Processing;
    }

    pub async fn set_stage_done(&self) {
        *self.stage.write().await = Stage::Done;
    }

    pub async fn stage(&self) -> RwLockReadGuard<Stage> {
        self.stage.read().await
    }

    pub async fn push_video(&self, video: Arc<Video>) {
        let mut videos = self.videos.write().await;
        (*videos).push(video);
    }

    pub async fn videos(&self) -> RwLockReadGuard<Vec<Arc<Video>>> {
        self.videos.read().await
    }

    pub async fn initiate_shutdown(&self) -> Result<()> {
        info!("Initiating shutdown.");

        // Set flag to refuse accepting new downloads (spawning new children).
        *self.stage.write().await = Stage::ShuttingDown;

        let videos = self.videos().await;

        // No need to terminate children and wait if all videos have finished downloading or failed.
        // Determine if interrupt and wait is necessary.
        let needs_interrupt = stream::iter((*videos).iter())
            .any(|video| async {
                // Is any download *not* yet finished or failed?
                // Then we will send an interrupt signal and wait for a few seconds
                // before reaping the remaining child processes.
                !matches!(
                    *video.stage().await,
                    self::video::Stage::Finished | self::video::Stage::Failed
                )
            })
            .await;

        if needs_interrupt {
            debug!("Sending SIGINT to child processes.");

            // Send SIGINT to all existing children.
            for video in (*videos).iter() {
                (*video).shutdown().await?;
            }

            // Then wait for 5 seconds for all children to self-terminate.
            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        Ok(())
    }

    pub async fn is_shutting_down(&self) -> bool {
        matches!(*self.stage.read().await, Stage::ShuttingDown)
    }
}
