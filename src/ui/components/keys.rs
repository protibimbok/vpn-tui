use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use crate::ui::theme::{ACCENT, MUTED};

pub fn key(label: &str) -> Span<'static> {
    Span::styled(
        format!(" {label} "),
        Style::default()
            .fg(Color::Black)
            .bg(ACCENT)
            .add_modifier(Modifier::BOLD),
    )
}

pub fn hint(text: &str) -> Span<'static> {
    Span::styled(text.to_string(), Style::default().fg(Color::Gray))
}

pub fn muted(text: &str) -> Span<'static> {
    Span::styled(text.to_string(), Style::default().fg(MUTED))
}
