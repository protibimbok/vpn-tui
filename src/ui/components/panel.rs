use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};

use crate::ui::theme::{ACCENT, MUTED};

/// Full-screen dark backdrop used by login cards.
pub fn fill_screen_bg(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(8, 12, 16))),
        area,
    );
}

/// Rounded accent card with a `VPN · {subtitle}` title and provider footer.
pub fn branded_panel(subtitle: &str, provider: &str) -> Block<'static> {
    let provider = provider.to_string();
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled("VPN", Style::default().fg(ACCENT).bold()),
            Span::raw(format!(" · {subtitle} ")),
        ]))
        .title_bottom(
            Line::from(vec![
                Span::raw(" "),
                Span::styled(provider, Style::default().fg(MUTED)),
                Span::raw(" "),
            ])
            .centered(),
        )
}
