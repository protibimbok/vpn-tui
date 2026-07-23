//! Proton TOTP entry after a successful password (SRP) step.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::{
    Store,
    state::Action,
    ui::{
        Component, branded_panel, fill_screen_bg, hint, key, render_input,
        theme::{ACCENT, FORM_WIDTH, MUTED},
    },
};

const FORM_HEIGHT: u16 = 12;
const HELP_HEIGHT: u16 = 4;

pub struct TwoFactor {
    code: String,
}

impl TwoFactor {
    pub fn new() -> Self {
        Self {
            code: String::new(),
        }
    }

    fn render_form(&self, frame: &mut Frame, area: Rect, state: &Store) {
        let block = branded_panel("Two-factor", state.provider().label());
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let [title, _, code_input, _, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .areas(inner);

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Enter the code from your authenticator app",
                Style::default().fg(MUTED),
            )))
            .centered(),
            title,
        );

        render_input(frame, code_input, &self.code, "Code", true, false);

        let status_line = if state.is_loading {
            Line::from(Span::styled(
                "Verifying…",
                Style::default().fg(ACCENT).add_modifier(Modifier::ITALIC),
            ))
        } else if let Some(err) = &state.error {
            Line::from(Span::styled(err.as_str(), Style::default().fg(Color::Red)))
        } else {
            Line::from(Span::styled(
                "Press Enter to verify",
                Style::default().fg(MUTED),
            ))
        };
        frame.render_widget(Paragraph::new(status_line).centered(), status);
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let help = Paragraph::new(vec![
            Line::from(vec![key("Enter"), hint(" verify")]),
            Line::from(vec![key("Esc"), hint(" back")]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED))
                .title(Line::from(Span::styled(
                    " Keys ",
                    Style::default().fg(MUTED),
                ))),
        );

        frame.render_widget(Clear, area);
        frame.render_widget(help, area);
    }
}

impl Component for TwoFactor {
    fn render(&mut self, frame: &mut Frame, area: Rect, state: &Store) {
        fill_screen_bg(frame, area);

        let content_height = FORM_HEIGHT + 1 + HELP_HEIGHT;
        let [content] = Layout::vertical([Constraint::Length(content_height)])
            .flex(Flex::Center)
            .areas(area);

        let form_width = FORM_WIDTH.min(area.width.saturating_sub(2).max(20));
        let [column] = Layout::horizontal([Constraint::Length(form_width)])
            .flex(Flex::Center)
            .areas(content);

        let [form_area, _, help_area] = Layout::vertical([
            Constraint::Length(FORM_HEIGHT),
            Constraint::Length(1),
            Constraint::Length(HELP_HEIGHT),
        ])
        .areas(column);

        self.render_form(frame, form_area, state);
        self.render_help(frame, help_area);
    }

    fn handle_input(&mut self, event: KeyEvent, _state: &Store) -> Action {
        match event.code {
            KeyCode::Enter => {
                if self.code.trim().is_empty() {
                    Action::None
                } else {
                    Action::Submit2fa {
                        code: self.code.clone(),
                    }
                }
            }
            KeyCode::Backspace => {
                self.code.pop();
                Action::None
            }
            KeyCode::Char('u') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.code.clear();
                Action::None
            }
            KeyCode::Char(c)
                if c.is_ascii_digit()
                    && self.code.len() < 8
                    && !event.modifiers.contains(KeyModifiers::CONTROL)
                    && !event.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.code.push(c);
                Action::Ignore
            }
            _ => Action::None,
        }
    }

    fn handle_mouse(&mut self, _event: MouseEvent, _state: &Store) -> Action {
        Action::None
    }

    fn update(&mut self, _state: &Store, _action: &Action) -> Action {
        Action::None
    }
}
