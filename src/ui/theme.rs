use ratatui::style::{Color, Style};

pub const ACCENT: Color = Color::Cyan;
pub const MUTED: Color = Color::DarkGray;
pub const QR_CODE_COLOR: Color = Color::Black;
pub const FORM_WIDTH: u16 = 48;
pub const FORM_HEIGHT: u16 = 15;
pub const HELP_HEIGHT: u16 = 6;

pub fn load_style(load: u32) -> Style {
    let color = match load {
        0..=39 => Color::Green,
        40..=69 => Color::Yellow,
        _ => Color::Red,
    };
    Style::new().fg(color)
}
