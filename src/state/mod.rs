use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard};

use self::video::Video;

pub mod video;

pub struct State {
    progress: RwLock<Progress>,
    videos: RwLock<Vec<Arc<Video>>>,
}

pub enum Progress {
    Initializing,
    FetchingSourcePage(String),
    ProcessingVideos,
}

impl State {
    pub fn new() -> Self {
        Self {
            progress: RwLock::new(Progress::Initializing),
            videos: RwLock::new(vec![]),
        }
    }

    pub async fn set_fetching_source_page<'a>(&self, page_url: &'a str) {
        *self.progress.write().await = Progress::FetchingSourcePage(page_url.to_string());
    }

    pub async fn set_processing_videos(&self) {
        *self.progress.write().await = Progress::ProcessingVideos;
    }

    pub async fn progress(&self) -> RwLockReadGuard<Progress> {
        self.progress.read().await
    }

    pub async fn push_video(&self, video: Arc<Video>) {
        let mut videos = self.videos.write().await;
        (*videos).push(video);
    }

    pub async fn videos(&self) -> RwLockReadGuard<Vec<Arc<Video>>> {
        self.videos.read().await
    }
}
