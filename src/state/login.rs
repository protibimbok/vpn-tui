use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use qrcode::{Color, QrCode};

pub struct CodeLogin {
    pub code: String,
    pub qr: Vec<String>,
    pub expires_at: Instant,
    /// Set to stop the background poller (on cancel, expiry, or success).
    cancel: Arc<AtomicBool>,
}

impl CodeLogin {
    pub fn new(code: String, ttl: Duration) -> Self {
        let qr = str_to_qr_code(&code);
        Self {
            code,
            qr,
            expires_at: Instant::now() + ttl,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Drop for CodeLogin {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Returns one string per text row. Each char packs the module above and
/// below it: `█` both dark, ` ` both light, `▀` top dark, `▄` bottom dark.
/// A one-module quiet zone is included on every side.
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
