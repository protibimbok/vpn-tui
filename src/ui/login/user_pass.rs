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
        theme::{ACCENT, FORM_HEIGHT, FORM_WIDTH, HELP_HEIGHT, MUTED},
    },
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Username,
    Password,
}

pub struct UserPass {
    username: String,
    password: String,
    focus: Focus,
}

impl UserPass {
    fn active_field_mut(&mut self) -> &mut String {
        match self.focus {
            Focus::Username => &mut self.username,
            Focus::Password => &mut self.password,
        }
    }

    fn cycle_focus(&mut self, reverse: bool) {
        self.focus = match (self.focus, reverse) {
            (Focus::Username, false) | (Focus::Password, true) => Focus::Password,
            (Focus::Password, false) | (Focus::Username, true) => Focus::Username,
        };
    }

    fn render_form(&self, frame: &mut Frame, area: Rect, state: &Store) {
        let block = branded_panel("Sign in");
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let [title, _, user_input, _, pass_input, _, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .areas(inner);

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Enter your credentials",
                Style::default().fg(MUTED),
            )))
            .centered(),
            title,
        );

        render_input(
            frame,
            user_input,
            &self.username,
            "Username",
            self.focus == Focus::Username,
            false,
        );

        render_input(
            frame,
            pass_input,
            &self.password,
            "Password",
            self.focus == Focus::Password,
            true,
        );

        let status_line = if state.is_loading {
            Line::from(Span::styled(
                "Signing in…",
                Style::default().fg(ACCENT).add_modifier(Modifier::ITALIC),
            ))
        } else if let Some(err) = &state.error {
            Line::from(Span::styled(err.as_str(), Style::default().fg(Color::Red)))
        } else {
            Line::from(Span::styled(
                "Press Enter to sign in",
                Style::default().fg(MUTED),
            ))
        };
        frame.render_widget(Paragraph::new(status_line).centered(), status);
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let help = Paragraph::new(vec![
            Line::from(vec![
                key("Tab"),
                Span::styled(" / ", Style::default().fg(MUTED)),
                key("Shift+Tab"),
                hint(" switch field"),
            ]),
            Line::from(vec![
                key("Esc"),
                hint(" quit"),
                Span::styled("  ·  ", Style::default().fg(MUTED)),
                key("Ctrl+U"),
                hint(" clear field"),
            ]),
            Line::from(vec![
                key("Alt+L"),
                hint(" log in with a code / QR"),
            ]),
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

impl UserPass {
    pub fn new(username: String) -> Self {
        let focus = if username.is_empty() {
            Focus::Username
        } else {
            Focus::Password
        };
        Self {
            username,
            password: String::new(),
            focus,
        }
    }
}

impl Component for UserPass {
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

    fn handle_input(&mut self, event: KeyEvent) -> Action {
        match event.code {
            KeyCode::Esc => Action::Quit,
            KeyCode::Tab => {
                self.cycle_focus(false);
                Action::Ignore
            }
            KeyCode::BackTab => {
                self.cycle_focus(true);
                Action::None
            }
            KeyCode::Enter => {
                if !self.username.is_empty() && !self.password.is_empty() {
                    Action::Login {
                        username: self.username.clone(),
                        password: self.password.clone(),
                    }
                } else {
                    self.cycle_focus(false);
                    Action::None
                }
            }
            KeyCode::Backspace => {
                self.active_field_mut().pop();
                Action::None
            }
            KeyCode::Char('u') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.active_field_mut().clear();
                Action::None
            }
            KeyCode::Char(c)
                if !event.modifiers.contains(KeyModifiers::CONTROL)
                    && !event.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.active_field_mut().push(c);
                Action::Ignore
            }
            _ => Action::None,
        }
    }

    fn handle_mouse(&mut self, _event: MouseEvent) -> Action {
        Action::None
    }

    fn update(&mut self, _state: &Store, _action: &Action) -> Action {
        Action::None
    }
}
