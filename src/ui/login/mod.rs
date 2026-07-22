mod user_pass;
mod qr;

use crate::{Store, state::Action, ui::{Component}};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::{Frame, layout::Rect};


use tokio::sync::mpsc::UnboundedSender;
use user_pass::*;
use qr::*;

enum LoginType {
    UserPass,
    QrCode,
}

pub struct LoginPage {
    login_type: LoginType,
    action_tx: UnboundedSender<Action>,
    /// Prefills the username field (from saved session email).
    email: String,
    component: Box<dyn Component>,
}

impl LoginPage {
    pub fn new(action_tx: UnboundedSender<Action>, email: String) -> Self {
        Self {
            login_type: LoginType::UserPass,
            action_tx: action_tx.clone(),
            component: Box::new(UserPass::new(email.clone())),
            email,
        }
    }

    fn user_pass(&self) -> Box<dyn Component> {
        Box::new(UserPass::new(self.email.clone()))
    }
}

impl Component for LoginPage {
    fn render(&mut self, frame: &mut Frame, area: Rect, state: &Store) {
        self.component.render(frame, area, state);
    }

    fn handle_input(&mut self, event: KeyEvent) -> Action {
        if event.code == KeyCode::Char('l') && event.modifiers.contains(KeyModifiers::ALT) {
            self.login_type = match self.login_type {
                LoginType::UserPass => LoginType::QrCode,
                LoginType::QrCode => LoginType::UserPass,
            };
            self.component = match self.login_type {
                LoginType::UserPass => self.user_pass(),
                LoginType::QrCode => Box::new(QrCode::new(self.action_tx.clone())),
            };
            // Leaving QR cancels the in-flight poller; entering starts SetQrCode.
            return match self.login_type {
                LoginType::UserPass => Action::CancelCodeLogin,
                LoginType::QrCode => Action::Ignore,
            };
        }
        if event.code == KeyCode::Esc && matches!(self.login_type, LoginType::QrCode) {
            self.login_type = LoginType::UserPass;
            self.component = self.user_pass();
            return Action::CancelCodeLogin;
        }
        let action = self.component.handle_input(event);
        if action != Action::None {
            return action;
        }
        Action::None
    }

    fn handle_mouse(&mut self, event: MouseEvent) -> Action {
        self.component.handle_mouse(event)
    }

    fn update(&mut self, state: &Store, action: &Action) -> Action {
        self.component.update(state, action)
    }
}
