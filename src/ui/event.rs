use crossterm::event::{KeyEvent, MouseEvent};

#[derive(Debug, Clone)]
pub enum UIEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
}