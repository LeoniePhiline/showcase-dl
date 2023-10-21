use std::rc::Rc;

use ratatui::layout::{Constraint, Direction, Layout, Rect};

use super::style;
use crate::state::video::VideoRead;

pub(crate) const CHUNKS_PER_VIDEO: usize = 4;

pub(crate) fn layout_chunks(size: Rect, videos: &[VideoRead]) -> Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(layout_constraints(videos))
        .split(size)
}

fn layout_constraints(videos: &[VideoRead]) -> Vec<Constraint> {
    let mut video_constraints = Vec::with_capacity(1 + videos.len() * 4 + 1); // TODO: Instead of re-allocating, place this vec in Ui struct - and only adjust its length as needed?

    // Application title block and table header, with bottom margin
    video_constraints.push(Constraint::Length(3));

    // Video gauge blocks
    for _ in videos {
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

pub(crate) fn video_raw_progress_table_layout() -> [Constraint; 4] {
    [
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(40),
        Constraint::Percentage(40), // 4-column span
    ]
}

pub(crate) fn video_progress_detail_table_layout() -> [Constraint; 7] {
    [
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(40),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
    ]
}
