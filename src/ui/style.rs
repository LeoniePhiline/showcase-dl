use tui::style::{Color, Modifier, Style};

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
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub fn video_title_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub fn gauge_style() -> Style {
    Style::default()
        .fg(Color::LightGreen)
        .add_modifier(Modifier::BOLD)
}
