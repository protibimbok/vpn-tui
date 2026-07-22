//! HTTP via system `curl` — avoids in-process TLS path-MTU issues behind the VPN.

use std::io::Write as _;
use std::process::{Command, Stdio};

const STATUS_MARK: &str = "\n\u{1}__protibimbok_vpn_status__\u{1}";

pub struct Response {
    pub status: u16,
    pub body: String,
}

pub struct Header<'a>(pub &'a str, pub &'a str);

pub fn get(url: &str, headers: &[Header]) -> Result<Response, String> {
    request("GET", url, headers, None)
}

pub fn post_json(url: &str, headers: &[Header], json: &str) -> Result<Response, String> {
    request("POST", url, headers, Some(json))
}

fn request(
    method: &str,
    url: &str,
    headers: &[Header],
    json_body: Option<&str>,
) -> Result<Response, String> {
    let mut cmd = Command::new("curl");
    cmd.arg("-sS")
        .args(["--connect-timeout", "10", "--max-time", "25"])
        .args(["-X", method])
        .args(["-w", &format!("{STATUS_MARK}%{{http_code}}")]);
    for Header(name, value) in headers {
        cmd.args(["-H", &format!("{name}: {value}")]);
    }
    if json_body.is_some() {
        cmd.args(["-H", "Content-Type: application/json"]);
        cmd.args(["--data-binary", "@-"]);
        cmd.stdin(Stdio::piped());
    }
    cmd.arg(url).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("cannot run curl (is it installed?): {e}"))?;
    if let Some(body) = json_body {
        child
            .stdin
            .take()
            .expect("stdin piped")
            .write_all(body.as_bytes())
            .map_err(|e| format!("writing request body: {e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("waiting on curl: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("network error: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let (body, status) = stdout
        .rsplit_once(STATUS_MARK)
        .ok_or_else(|| "malformed curl output".to_string())?;
    let status = status
        .trim()
        .parse()
        .map_err(|_| format!("unexpected status from curl: {status:?}"))?;
    Ok(Response {
        status,
        body: body.to_string(),
    })
}
