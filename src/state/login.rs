use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use qrcode::{Color, QrCode};
use tokio::sync::mpsc::UnboundedSender;

use crate::api::surfshark::{self, LOGIN_CODE_URL};

use super::Action;

const CODE_POLL_INTERVAL: Duration = Duration::from_secs(5);

pub struct CodeLogin {
    pub code: String,
    pub qr: Vec<String>,
    pub expires_at: Instant,
    /// Set to stop the background poller (on cancel, expiry, or success).
    cancel: Arc<AtomicBool>,
}

impl CodeLogin {
    /// Builds the on-screen challenge and returns a cancel handle the caller
    /// clones into the background poller.
    pub fn new(code: String, ttl: Duration) -> (Self, Arc<AtomicBool>) {
        let qr = str_to_qr_code(&format!("{LOGIN_CODE_URL}{code}"));
        let cancel = Arc::new(AtomicBool::new(false));
        (
            Self {
                code,
                qr,
                expires_at: Instant::now() + ttl,
                cancel: cancel.clone(),
            },
            cancel,
        )
    }
}

impl Drop for CodeLogin {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}


pub fn poll_login_code(tx: &UnboundedSender<Action>, hash: &str, cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        // Sleep in small slices so cancellation is felt promptly.
        for _ in 0..(CODE_POLL_INTERVAL.as_millis() / 100) {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        match surfshark::poll_login_code(hash) {
            Ok(surfshark::PollResult::Pending) => {}
            Ok(surfshark::PollResult::Approved(tokens)) => {
                let _ = tx.send(Action::LoggedIn {
                    token: tokens.token,
                    renew_token: tokens.renew_token,
                    uid: None,
                    email: None,
                });
                return;
            }
            Err(e) => {
                let _ = tx.send(Action::Error(e.to_string()));
                return;
            }
        }
    }
}


pub fn str_to_qr_code(data: &str) -> Vec<String> {
    let Ok(code) = QrCode::new(data.as_bytes()) else {
        return Vec::new();
    };
    let width = code.width();
    let modules = code.to_colors();
    let dark = |x: usize, y: usize| -> bool {
        x < width && y < width && modules[y * width + x] == Color::Dark
    };

    let quiet = 1;
    let size = width + quiet * 2;
    let mut rows = Vec::with_capacity(size.div_ceil(2));
    for row in (0..size).step_by(2) {
        let mut line = String::with_capacity(size);
        for col in 0..size {
            let x = col.wrapping_sub(quiet);
            let top = row >= quiet && dark(x, row - quiet);
            let bottom = row + 1 >= quiet && dark(x, row + 1 - quiet);
            line.push(match (top, bottom) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        rows.push(line);
    }
    rows
}
