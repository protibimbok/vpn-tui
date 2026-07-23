//! Login screen: password, Surfshark QR, and Proton 2FA.

mod user_pass;
mod qr;
mod two_factor;

use crate::{
    Store,
    api::Provider,
    state::Action,
    ui::Component,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::{Frame, layout::Rect};
use tokio::sync::mpsc::UnboundedSender;

use user_pass::*;
use qr::*;
use two_factor::*;

enum LoginType {
    UserPass,
    QrCode,
    TwoFactor,
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

    fn show_user_pass(&mut self) {
        self.login_type = LoginType::UserPass;
        self.component = self.user_pass();
    }

    fn show_qr(&mut self) {
        self.login_type = LoginType::QrCode;
        self.component = Box::new(QrCode::new(self.action_tx.clone()));
    }

    fn show_2fa(&mut self) {
        self.login_type = LoginType::TwoFactor;
        self.component = Box::new(TwoFactor::new());
    }
}

impl Component for LoginPage {
    fn render(&mut self, frame: &mut Frame, area: Rect, state: &Store) {
        self.component.render(frame, area, state);
    }

    fn handle_input(&mut self, event: KeyEvent, state: &Store) -> Action {
        // Surfshark-only: Alt+L toggles QR login.
        if event.code == KeyCode::Char('l')
            && event.modifiers.contains(KeyModifiers::ALT)
            && state.provider() == Provider::Surfshark
            && !matches!(self.login_type, LoginType::TwoFactor)
        {
            match self.login_type {
                LoginType::UserPass => {
                    self.show_qr();
                    return Action::Ignore;
                }
                LoginType::QrCode => {
                    self.show_user_pass();
                    return Action::CancelCodeLogin;
                }
                LoginType::TwoFactor => {}
            }
        }

        if event.code == KeyCode::Esc && matches!(self.login_type, LoginType::QrCode) {
            self.show_user_pass();
            return Action::CancelCodeLogin;
        }

        if event.code == KeyCode::Esc && matches!(self.login_type, LoginType::TwoFactor) {
            self.show_user_pass();
            return Action::Cancel2fa;
        }

        let action = self.component.handle_input(event, state);
        if action != Action::None {
            return action;
        }
        Action::None
    }

    fn handle_mouse(&mut self, event: MouseEvent, state: &Store) -> Action {
        self.component.handle_mouse(event, state)
    }

    fn update(&mut self, state: &Store, action: &Action) -> Action {
        // Provider switch rebuilds the password form with the other email.
        if matches!(action, Action::SwitchProvider) {
            self.email = state.email().to_string();
            self.show_user_pass();
            return Action::None;
        }
        // Enter 2FA screen when Proton requests it.
        if matches!(action, Action::TwoFactorRequired { .. }) {
            self.show_2fa();
            return Action::None;
        }
        // Leaving 2FA / successful login returns to password form.
        if matches!(action, Action::Cancel2fa | Action::LoggedIn { .. })
            && matches!(self.login_type, LoginType::TwoFactor)
        {
            self.show_user_pass();
        }
        self.component.update(state, action)
    }
}
