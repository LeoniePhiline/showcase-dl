use tokio::sync::RwLockReadGuard;
use tui::layout::{Constraint, Direction, Layout, Rect};

use super::style;
use crate::state::video::VideoRead;

pub const CHUNKS_PER_VIDEO: usize = 4;

pub fn layout_chunks(size: Rect, videos: &RwLockReadGuard<Vec<VideoRead>>) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(layout_constraints(videos))
        .split(size)
}

fn layout_constraints(videos: &RwLockReadGuard<Vec<VideoRead>>) -> Vec<Constraint> {
    let mut video_constraints = Vec::with_capacity(videos.len()); // TODO: Instead of re-allocating, place this vec in Ui struct - and only adjust its length as needed?

    // Application title block and table header
    video_constraints.push(Constraint::Length(2));

    // Video gauge blocks
    for _ in videos.iter() {
        // Video header block
        video_constraints.push(Constraint::Length(1));
        // Video progress text
        video_constraints.push(Constraint::Length(1));
        // Video progress bar
        video_constraints.push(Constraint::Length(1));
        // Video bottom margin
        video_constraints.push(Constraint::Length(style::SPACE_Y));
    }

    video_constraints.push(Constraint::Min(0));

    video_constraints
}

pub fn video_progress_table_layout() -> [Constraint; 5] {
    [
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ]
}
