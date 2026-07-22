use crate::{Action, Store, ui::theme::ACCENT};
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect, style::Style, widgets::Block};
use super::Component;

pub struct ServersPage {}

impl ServersPage {
    pub fn new() -> Self {
        Self {}
    }
}

impl Component for ServersPage {
    fn render(&mut self, frame: &mut Frame, area: Rect, state: &Store) {
        let block = Block::default().title("Servers").border_style(Style::default().fg(ACCENT));
        frame.render_widget(block, area);
    }

    fn handle_input(&mut self, event: KeyEvent) -> Action {
        Action::None
    }

    fn handle_mouse(&mut self, event: MouseEvent) -> Action {
        Action::None
    }

    fn update(&mut self, state: &Store, action: &Action) -> Action {
        Action::None
    }
}