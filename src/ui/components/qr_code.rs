use ratatui::{Frame, layout::Rect, style::Style, widgets::Paragraph};

use crate::ui::theme::QR_CODE_COLOR;

pub fn render_qr_code(frame: &mut Frame, area: Rect, qr_code: &[String]) {
    let qr_w = qr_code
        .first()
        .map(|r| r.chars().count() as u16)
        .unwrap_or(0)
        .min(area.width);
    let x = area.x + area.width.saturating_sub(qr_w) / 2;

    for (i, line) in qr_code.iter().enumerate() {
        if i as u16 >= area.height {
            break;
        }
        frame.render_widget(
            Paragraph::new(line.as_str()).style(
                Style::default()
                    .fg(QR_CODE_COLOR)
                    .bg(ratatui::style::Color::White),
            ),
            Rect::new(x, area.y + i as u16, qr_w, 1),
        );
    }
}
