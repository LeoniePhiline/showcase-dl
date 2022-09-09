use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard};

use self::video::Video;

pub mod video;

pub struct State {
    stage: RwLock<Stage>,
    videos: RwLock<Vec<Arc<Video>>>,
}

pub enum Stage {
    Initializing,
    FetchingSource(String),
    Processing,
    // TODO: Semantic detail: Rename to `Finished` or keep at `Done`?
    Done,
}

impl State {
    pub fn new() -> Self {
        Self {
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
}
