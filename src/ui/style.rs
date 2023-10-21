use ratatui::style::{Color, Modifier, Style};

use crate::state::video::Stage;

pub(crate) const SPACE_Y: u16 = 1;

#[inline]
pub(crate) fn application_title_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub(crate) fn border_style() -> Style {
    Style::default().fg(Color::LightBlue)
}

#[inline]
pub(crate) fn table_header_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub(crate) fn video_title_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub(crate) fn video_stage_style(video_stage: Stage) -> Style {
    Style::default()
        .fg(video_stage_color(video_stage))
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub(crate) fn gauge_style(video_stage: Stage) -> Style {
    Style::default()
        .fg(video_stage_color(video_stage))
        .add_modifier(Modifier::BOLD)
}

fn video_stage_color(video_stage: Stage) -> Color {
    match video_stage {
        Stage::Initializing => Color::LightCyan,
        Stage::Running { .. } => Color::LightYellow,
        Stage::ShuttingDown { .. } => Color::LightBlue,
        Stage::Finished => Color::LightGreen,
        Stage::Failed => Color::LightRed,
    }
}
