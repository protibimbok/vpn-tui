//! Drawing for the servers screen.

use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
};

use crate::{
    Store,
    api::Server,
    ui::{
        fill_screen_bg, hint, key, muted,
        theme::{load_style, ACCENT, MUTED},
    },
};

use super::list::{Density, ServerList};

impl ServerList {
    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, state: &Store) {
        self.recompute(state);
        fill_screen_bg(frame, area);

        let help = self.help_lines(area.width);
        let help_h = help.len() as u16;
        // Keep status (3) + list (≥3); remaining goes to help + feedback.
        let max_footer = area.height.saturating_sub(6);
        let footer_h = (help_h + 1).min(max_footer).max(1);
        let help_rows = footer_h.saturating_sub(1).min(help_h);

        let [status_area, list_area, footer_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(footer_h),
        ])
        .areas(area);

        render_status(frame, status_area, state);
        self.render_table(frame, list_area, state);

        if help_rows == 0 {
            frame.render_widget(Paragraph::new(feedback_line(state)), footer_area);
        } else {
            let [help_area, feedback_area] = Layout::vertical([
                Constraint::Length(help_rows),
                Constraint::Length(1),
            ])
            .areas(footer_area);

            let shown: Vec<Line<'static>> = help.into_iter().take(help_rows as usize).collect();
            frame.render_widget(Paragraph::new(shown), help_area);
            frame.render_widget(Paragraph::new(feedback_line(state)), feedback_area);
        }
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect, state: &Store) {
        let density = Density::from_width(area.width);

        // Borders (2) + header (1) = rows that fit in the body.
        let page_size = area.height.saturating_sub(3).max(1) as usize;
        self.sync_scroll(page_size);

        let total = self.visible.len();
        let end = (self.scroll + page_size).min(total);
        let window = if self.scroll < end {
            &self.visible[self.scroll..end]
        } else {
            &[][..]
        };

        let range = if total == 0 {
            String::new()
        } else {
            format!(" · {}–{}", self.scroll + 1, end)
        };
        let title = format!(
            " Servers ({total}){range} · sort: {}{} ",
            self.sort.label(),
            if self.filter.is_empty() {
                String::new()
            } else {
                format!(" · search: {}", self.filter)
            }
        );

        let (headers, constraints) = columns(density);
        let header = Row::new(headers.into_iter().map(Cell::from).collect::<Vec<_>>())
            .style(Style::new().add_modifier(Modifier::BOLD).fg(ACCENT));

        // Only materialize rows in the scroll window — Proton lists can be thousands.
        let rows = window.iter().map(|&i| {
            let s = &state.servers[i];
            Row::new(row_cells(density, state, s))
        });

        let table = Table::new(rows, constraints)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(ACCENT))
                    .title(Line::from(Span::styled(title, Style::default().fg(ACCENT)))),
            )
            .row_highlight_style(Style::new().bg(ACCENT).fg(Color::Black))
            .highlight_symbol("▶ ");

        // Selection is absolute in `table_state`; map to the window-local index.
        let local = self
            .table_state
            .selected()
            .map(|sel| sel.saturating_sub(self.scroll));
        let mut window_state = TableState::default().with_selected(local);
        frame.render_stateful_widget(table, area, &mut window_state);
    }

    fn help_lines(&self, width: u16) -> Vec<Line<'static>> {
        if self.filtering {
            return vec![Line::from(vec![
                Span::styled(" /", Style::default().fg(ACCENT)),
                Span::raw(self.filter.clone()),
                Span::styled("▌", Style::default().fg(ACCENT)),
                muted("  ↑/↓ move · Enter connect · Esc clear"),
            ])];
        }
        wrap_help(HELP_ITEMS, width)
    }
}

/// Full help set; wrapped to terminal width instead of abbreviated.
const HELP_ITEMS: &[(&str, &str)] = &[
    ("↑/↓", " select"),
    ("Enter", " connect"),
    ("f", " fastest"),
    ("p", " ping all"),
    ("d", " disconnect"),
    ("s", " sort"),
    ("/", " search"),
    ("r", " refresh"),
    ("L", " logout"),
    ("Alt+Space", " provider"),
    ("q", " quit"),
];

fn wrap_help(items: &[(&str, &str)], width: u16) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0u16;

    for &(label, desc) in items {
        // `key()` renders as ` {label} `
        let item_w = (label.chars().count() as u16).saturating_add(2 + desc.chars().count() as u16);
        let gap = if spans.is_empty() { 0 } else { 2 };
        if !spans.is_empty() && used.saturating_add(gap + item_w) > width {
            lines.push(Line::from(std::mem::take(&mut spans)));
            used = 0;
        }
        if !spans.is_empty() {
            spans.push(hint("  "));
            used = used.saturating_add(2);
        }
        spans.push(key(label));
        spans.push(hint(desc));
        used = used.saturating_add(item_w);
    }
    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines
}

fn columns(density: Density) -> (Vec<&'static str>, Vec<Constraint>) {
    match density {
        Density::Compact => (
            vec!["Title", "Ping", "Load", "●"],
            vec![
                Constraint::Min(12),
                Constraint::Length(7),
                Constraint::Length(5),
                Constraint::Length(3),
            ],
        ),
        Density::Comfortable => (
            vec!["Title", "Load", "Ping", "Connected"],
            vec![
                Constraint::Min(18),
                Constraint::Length(6),
                Constraint::Length(8),
                Constraint::Length(10),
            ],
        ),
        Density::Wide => (
            vec!["Title", "Load", "Ping", "Connected"],
            vec![
                Constraint::Min(28),
                Constraint::Length(6),
                Constraint::Length(8),
                Constraint::Length(10),
            ],
        ),
    }
}

fn row_cells(density: Density, state: &Store, s: &Server) -> Vec<Cell<'static>> {
    let ping = match state.latencies.get(&s.endpoint_host) {
        Some(Some(ms)) => format!("{ms} ms"),
        Some(None) => "—".into(),
        None => String::new(),
    };
    let is_conn = ServerList::is_connected(state, s);
    let conn_cell = if is_conn {
        Cell::from("● yes").style(Style::new().fg(Color::Green))
    } else {
        Cell::from("○").style(Style::new().fg(MUTED))
    };
    let load = Cell::from(format!("{}%", s.load)).style(load_style(s.load));

    match density {
        Density::Compact => vec![
            Cell::from(s.connected_label()),
            Cell::from(ping),
            load,
            if is_conn {
                Cell::from("●").style(Style::new().fg(Color::Green))
            } else {
                Cell::from(" ").style(Style::new().fg(MUTED))
            },
        ],
        Density::Comfortable => vec![
            Cell::from(s.connected_label()),
            load,
            Cell::from(ping),
            conn_cell,
        ],
        Density::Wide => vec![
            Cell::from(s.connected_label()),
            load,
            Cell::from(ping),
            conn_cell,
        ],
    }
}

fn render_status(frame: &mut Frame, area: Rect, state: &Store) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled("VPN", Style::default().fg(ACCENT).bold()),
            Span::raw(format!(" · {} ", state.provider().label())),
        ]));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(connection_line(state, inner.width)),
        inner,
    );
}

fn connection_line(state: &Store, width: u16) -> Line<'static> {
    match &state.connected {
        Some(stored) => {
            let name = state
                .servers
                .iter()
                .find(|s| ServerList::is_connected(state, s))
                .map(Server::connected_label)
                .unwrap_or_else(|| stored.clone());
            let mut text = if width < 60 {
                format!("● {name}")
            } else {
                format!("● Connected — {name}")
            };
            if width >= 90
                && let Some(st) = &state.wg_status
            {
                if !st.endpoint.is_empty() {
                    text.push_str(&format!(" · {}", st.endpoint));
                }
                text.push_str(&format!(
                    " · ↓ {} ↑ {}",
                    human_bytes(st.rx),
                    human_bytes(st.tx),
                ));
                if let Some(handshake) = st.handshake_unix {
                    text.push_str(&format!(" · handshake {}", handshake_age(handshake)));
                }
            }
            Line::styled(text, Style::new().fg(Color::Green))
        }
        None => Line::styled("○ Disconnected", Style::new().fg(MUTED)),
    }
}

fn feedback_line(state: &Store) -> Line<'static> {
    if let Some(busy) = &state.busy {
        Line::from(Span::styled(
            busy.clone(),
            Style::default().fg(ACCENT).add_modifier(Modifier::ITALIC),
        ))
    } else if let Some(err) = &state.error {
        Line::from(Span::styled(err.clone(), Style::default().fg(Color::Red)))
    } else if let Some(msg) = &state.status_msg {
        Line::from(Span::styled(msg.clone(), Style::default().fg(MUTED)))
    } else {
        Line::from("")
    }
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn handshake_age(unix: u64) -> String {
    if unix == 0 {
        return "pending".into();
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(unix);
    let age = now.saturating_sub(unix);
    if age < 60 {
        format!("{age}s ago")
    } else {
        format!("{}m ago", age / 60)
    }
}
