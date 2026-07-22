//! Latency probing via the system `ping` binary (works without raw-socket
//! privileges, unlike ICMP from within the process).

use std::process::Command;

/// Round-trip time to `host` in milliseconds, or `None` on timeout/failure.
pub fn ping_ms(host: &str) -> Option<u32> {
    let out = Command::new("ping")
        .args(["-n", "-c", "1", "-W", "1", host])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let rest = &text[text.find("time=")? + 5..];
    let value = rest.split_whitespace().next()?;
    value.parse::<f32>().ok().map(|ms| ms.round() as u32)
}
