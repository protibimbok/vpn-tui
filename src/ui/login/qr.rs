use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
};
use std::time::Instant;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    Store,
    state::Action,
    ui::{
        Component, branded_panel, fill_screen_bg, hint, key, muted, render_qr_code,
        theme::{ACCENT, MUTED},
    },
};

pub struct QrCode;

impl QrCode {
    pub fn new(action_tx: UnboundedSender<Action>) -> Self {
        let _ = action_tx.send(Action::SetQrCode);
        Self
    }

    fn render_card(&self, frame: &mut Frame, area: Rect, state: &Store) {
        let qr_w = state
            .code_login
            .as_ref()
            .and_then(|c| c.qr.first())
            .map(|r| r.chars().count() as u16)
            .unwrap_or(0);
        
        
        let qr_rows = state
            .code_login
            .as_ref()
            .map(|c| c.qr.len() as u16)
            .unwrap_or(0)
            .max(1);

        let width = (qr_w + 8).clamp(50, area.width);
        // intro(2) + qr + code(1) + expiry(1) + feedback(1) + foot(1) + borders(2)
        let height = (qr_rows + 8).min(area.height);

        let [card_area] = Layout::horizontal([Constraint::Length(width)])
            .flex(Flex::Center)
            .areas(area);
        let [card_area] = Layout::vertical([Constraint::Length(height)])
            .flex(Flex::Center)
            .areas(card_area);

        let block = branded_panel("Log in with a code");
        let body = block.inner(card_area);
        frame.render_widget(Clear, card_area);
        frame.render_widget(block, card_area);

        let [intro, qr_area, code_area, expiry, feedback, foot] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(qr_rows),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .horizontal_margin(2)
        .areas(body);

        frame.render_widget(
            Paragraph::new(
                "Scan the QR, or open the Surfshark app → My account → Enter login code:",
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(MUTED)),
            intro,
        );

        if let Some(code) = &state.code_login {
            render_qr_code(frame, qr_area, &code.qr);

            frame.render_widget(
                Paragraph::new(Line::from(vec![Span::styled(
                    spaced(&code.code),
                    Style::default().fg(ACCENT).bold(),
                )]))
                .alignment(Alignment::Center),
                code_area,
            );

            let secs = code
                .expires_at
                .saturating_duration_since(Instant::now())
                .as_secs();
            frame.render_widget(
                Paragraph::new(format!("expires in {}:{:02}", secs / 60, secs % 60))
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(MUTED)),
                expiry,
            );
        } else {
            frame.render_widget(
                Paragraph::new("requesting code…")
                    .alignment(Alignment::Center)
                    .style(
                        Style::default()
                            .fg(ACCENT)
                            .add_modifier(Modifier::ITALIC),
                    ),
                qr_area,
            );
        }

        let feedback_line = if state.is_loading && state.code_login.is_none() {
            Line::from(Span::styled(
                "Waiting for login code…",
                Style::default().fg(ACCENT).add_modifier(Modifier::ITALIC),
            ))
        } else if let Some(err) = &state.error {
            Line::from(Span::styled(err.as_str(), Style::default().fg(Color::Red)))
        } else {
            Line::from(Span::styled(
                "Waiting for approval in the app…",
                Style::default().fg(MUTED),
            ))
        };
        frame.render_widget(
            Paragraph::new(feedback_line)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            feedback,
        );

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                key("Esc"),
                hint(" back"),
                muted(" · "),
                key("r"),
                hint(" new code"),
                muted(" · "),
                key("Ctrl+C"),
                hint(" quit"),
            ]))
            .alignment(Alignment::Center),
            foot,
        );
    }
}

fn spaced(s: &str) -> String {
    s.chars()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

impl Component for QrCode {
    fn render(&mut self, frame: &mut Frame, area: Rect, state: &Store) {
        fill_screen_bg(frame, area);
        self.render_card(frame, area, state);
    }

    fn handle_input(&mut self, event: KeyEvent, _state: &Store) -> Action {
        match event.code {
            KeyCode::Char('r') => Action::SetQrCode,
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
