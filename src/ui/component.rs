use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};

use crate::{Action, Store};

pub trait Component {
    fn render(&mut self, frame: &mut Frame, area: Rect, state: &Store);

    fn handle_input(&mut self, event: KeyEvent, state: &Store) -> Action;
    fn handle_mouse(&mut self, event: MouseEvent, state: &Store) -> Action;

    fn update(&mut self, state: &Store, action: &Action) -> Action;
}
