use tui::style::{Color, Modifier, Style};

use crate::state::video::Stage;

pub const SPACE_Y: u16 = 0;

#[inline]
pub fn application_title_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub fn border_style() -> Style {
    Style::default().fg(Color::LightBlue)
}

#[inline]
pub fn table_header_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub fn video_title_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub fn video_stage_style(video_stage: &Stage) -> Style {
    Style::default()
        .fg(video_stage_color(video_stage))
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub fn gauge_style(video_stage: &Stage) -> Style {
    Style::default()
        .fg(video_stage_color(video_stage))
        .add_modifier(Modifier::BOLD)
}

fn video_stage_color(video_stage: &Stage) -> Color {
    match video_stage {
        Stage::Initializing => Color::LightCyan,
        Stage::Downloading => Color::LightYellow,
        Stage::Finished => Color::LightGreen,
    }
}
