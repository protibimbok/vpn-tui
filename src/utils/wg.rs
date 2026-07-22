//! WireGuard: config rendering and a thin wrapper around `wg-quick` / `wg`.
//! Root is obtained when already root, otherwise via passwordless `sudo -n`
//! (a password prompt would corrupt the TUI). Status queries never use sudo.

use std::fs;
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::Command;

use crate::api::Server;

pub const IFACE: &str = "vpn";
const SURFSHARK_DNS: &str = "162.252.172.57, 149.154.159.92";
const WG_PORT: u16 = 51820;

pub type Result<T> = std::result::Result<T, String>;

fn root_command(args: &[&str]) -> Command {
    if geteuid() == 0 {
        let mut cmd = Command::new(args[0]);
        cmd.args(&args[1..]);
        cmd
    } else {
        let mut cmd = Command::new("sudo");
        cmd.arg("-n").args(args);
        cmd
    }
}

fn geteuid() -> u32 {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid has no preconditions and always succeeds.
    unsafe { geteuid() }
}

pub fn write_conf(path: &Path, private_key: &str, server: &Server) -> Result<()> {
    let conf = format!(
        "[Interface]\n\
         PrivateKey = {private_key}\n\
         Address = 10.14.0.2/16\n\
         DNS = {SURFSHARK_DNS}\n\
         \n\
         [Peer]\n\
         PublicKey = {}\n\
         AllowedIPs = 0.0.0.0/0, ::/0\n\
         Endpoint = {}:{WG_PORT}\n",
        server.wg_public_key, server.endpoint_host
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    file.write_all(conf.as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(())
}

pub fn up(conf: &Path) -> Result<()> {
    run(&["wg-quick", "up", &conf.to_string_lossy()]).map(|_| ())
}

pub fn down(conf: &Path) -> Result<()> {
    run(&["wg-quick", "down", &conf.to_string_lossy()]).map(|_| ())
}

#[derive(Clone, Debug)]
pub struct Status {
    pub endpoint: String,
    /// `None` when `wg show` isn't available unprivileged (sysfs fallback).
    pub handshake_unix: Option<u64>,
    pub rx: u64,
    pub tx: u64,
}

/// `None` when the interface is down.
pub fn status(conf: &Path) -> Option<Status> {
    let sys = Path::new("/sys/class/net").join(IFACE);
    if !sys.exists() {
        return None;
    }
    if let Some(status) = wg_dump() {
        return Some(status);
    }
    let counter = |name: &str| -> u64 {
        fs::read_to_string(sys.join("statistics").join(name))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    };
    Some(Status {
        endpoint: conf_endpoint(conf),
        handshake_unix: None,
        rx: counter("rx_bytes"),
        tx: counter("tx_bytes"),
    })
}

fn wg_dump() -> Option<Status> {
    if geteuid() != 0 {
        return None;
    }
    let out = root_command(&["wg", "show", IFACE, "dump"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let out = String::from_utf8_lossy(&out.stdout);
    let fields: Vec<&str> = out.lines().nth(1)?.split('\t').collect();
    if fields.len() < 7 {
        return None;
    }
    Some(Status {
        endpoint: fields[2].to_string(),
        handshake_unix: Some(fields[4].parse().unwrap_or(0)),
        rx: fields[5].parse().unwrap_or(0),
        tx: fields[6].parse().unwrap_or(0),
    })
}

fn conf_endpoint(conf: &Path) -> String {
    fs::read_to_string(conf)
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|l| l.strip_prefix("Endpoint = ").map(|e| e.trim().to_string()))
        })
        .unwrap_or_default()
}

fn run(args: &[&str]) -> Result<String> {
    let output = root_command(args)
        .output()
        .map_err(|e| format!("failed to spawn `{}`: {e}", args[0]))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut msg = format!("`{}`: {}", args.join(" "), stderr.trim());
        if msg.contains("password is required") {
            msg.push_str(
                " — run as root or add a passwordless sudo rule for wg-quick",
            );
        }
        return Err(msg);
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
