//! WireGuard: config rendering and a thin wrapper around `wg-quick` / `wg`.
//! Root is obtained per child process: directly when already root, by
//! re-elevating from the saved uid when the binary is installed setuid-root
//! (see [`drop_setuid_root`]), or through passwordless sudo as a last resort.
//! Status queries never use sudo: they read sysfs and our own conf instead.

use std::fs;
use std::io::{self, Write as _};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt as _;
use std::path::Path;
use std::process::Command;

use crate::api::{Provider, Server};

pub const IFACE: &str = "vpn";
const SURFSHARK_DNS: &str = "162.252.172.57, 149.154.159.92";
const PROTON_DNS: &str = "10.2.0.1";
const WG_PORT: u16 = 51820;

pub type Result<T> = std::result::Result<T, String>;

/// Call first thing in `main`, before any threads or files: when installed
/// setuid-root, drop the effective uid back to the invoking user so the app
/// itself (config files, network, workers) runs unprivileged. Root stays in
/// the saved uid, from which [`root_command`] re-elevates individual
/// `wg`/`wg-quick` children.
pub fn drop_setuid_root() {
    let ruid = unsafe { libc::getuid() };
    if unsafe { libc::geteuid() } == 0 && ruid != 0 {
        // Saved uid is already 0 from the setuid exec; only euid changes.
        unsafe { libc::setresuid(!0, ruid, !0) };
    }
}

/// Whether the saved uid is root, i.e. we were started setuid-root and
/// children can re-elevate without sudo.
pub fn saved_root() -> bool {
    let (mut ruid, mut euid, mut suid) = (0, 0, 0);
    unsafe { libc::getresuid(&mut ruid, &mut euid, &mut suid) };
    suid == 0
}

/// Builds a command that will run as root: directly when we are root,
/// re-elevating in the child when setuid-installed, else via `sudo -n`
/// (-n: never prompt; a password prompt would corrupt the TUI).
fn root_command(args: &[&str]) -> Command {
    if unsafe { libc::geteuid() } == 0 {
        let mut cmd = Command::new(args[0]);
        cmd.args(&args[1..]);
        cmd
    } else if saved_root() {
        let mut cmd = Command::new(args[0]);
        cmd.args(&args[1..]);
        // Don't run root children with the caller's PATH.
        cmd.env(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        );
        unsafe {
            cmd.pre_exec(|| {
                // Raw syscall: glibc's wrapper isn't async-signal-safe here.
                if libc::syscall(libc::SYS_setresuid, 0, 0, 0) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        cmd
    } else {
        let mut cmd = Command::new("sudo");
        cmd.arg("-n").args(args);
        cmd
    }
}

pub fn write_conf(
    path: &Path,
    provider: Provider,
    private_key: &str,
    server: &Server,
) -> Result<()> {
    // Surfshark hands out a /16 and its own DNS; Proton uses a /32 and 10.2.0.1.
    let (dns, address) = match provider {
        Provider::Surfshark => (SURFSHARK_DNS, "10.14.0.2/16"),
        Provider::Proton => (PROTON_DNS, "10.2.0.2/32"),
    };
    let conf = format!(
        "[Interface]\n\
         PrivateKey = {private_key}\n\
         Address = {address}\n\
         DNS = {dns}\n\
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

/// `None` when the interface is down. `wg show` needs CAP_NET_ADMIN, so it
/// runs only when root comes free (already root, or setuid-installed);
/// otherwise traffic counters come from sysfs and the endpoint from our own
/// conf, which covers everything except the handshake time.
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
    // No sudo here: this runs every few seconds and a `sudo` per poll would
    // spam the auth log. Without root available the sysfs fallback covers it.
    if unsafe { libc::geteuid() } != 0 && !saved_root() {
        return None;
    }
    let out = root_command(&["wg", "show", IFACE, "dump"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let out = String::from_utf8_lossy(&out.stdout);
    // Line 0 is the interface, line 1 the single peer:
    // pubkey  psk  endpoint  allowed-ips  latest-handshake  rx  tx  keepalive
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
    conf_field(conf, "Endpoint = ").unwrap_or_default()
}

/// Peer `PublicKey` from our written conf — unique even when Proton EntryIPs are shared.
pub fn conf_peer_public_key(conf: &Path) -> Option<String> {
    conf_field(conf, "PublicKey = ")
}

fn conf_field(conf: &Path, prefix: &str) -> Option<String> {
    fs::read_to_string(conf).ok().and_then(|text| {
        text.lines()
            .find_map(|l| l.strip_prefix(prefix).map(|v| v.trim().to_string()))
    })
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
                " — run vpn as root, install it setuid (see README), \
                 or add a passwordless sudo rule for wg-quick",
            );
        }
        return Err(msg);
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
