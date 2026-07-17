use ratatui::{widgets::{Block, Paragraph}, prelude::*};
use crate::ui::theme::{ACCENT, MUTED};
use ratatui::widgets::{Borders, BorderType};

pub fn render_input(
    frame: &mut Frame,
    area: Rect,
    value: &str,
    label: &str,
    focused: bool,
    masked: bool,
) {
    let color = if focused { ACCENT } else { MUTED };
    let border = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(label, Style::default().fg(color).bold()),
            Span::raw(" "),
        ]));

        let display: Line = if masked {
            let masked = Masked::new(value, '•');
            let mut line = Line::from(masked.to_string());
            if focused {
                line.push_span(Span::styled("█", Style::default().fg(color)));
            }
            line
        } else {
            let mut spans = vec![Span::raw(value)];
            if focused {
                spans.push(Span::styled("█", Style::default().fg(color)));
            } else if value.is_empty() {
                spans = vec![Span::styled(
                    "email@example.com",
                    Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
                )];
            }
            Line::from(spans)
        };

        frame.render_widget(
            Paragraph::new(display)
                .style(Style::default().fg(color))
                .block(border),
            area,
        );
}