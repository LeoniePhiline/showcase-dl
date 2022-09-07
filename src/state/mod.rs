use std::sync::Arc;
use tokio::sync::RwLock;

use self::video::Video;

pub mod video;

pub struct State {
    videos: RwLock<Vec<Arc<Video>>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            videos: RwLock::new(vec![]),
        }
    }

    pub async fn push_video(&self, video: Arc<Video>) {
        let mut videos = self.videos.write().await;
        (*videos).push(video);
    }
}
