//! Pre-flight check for external binaries the TUI shells out to.

use std::io::{self, Write as _};
use std::process::{Command, Stdio};

struct Dep {
    bin: &'static str,
    /// Package names: (pacman, apt, dnf).
    packages: (&'static str, &'static str, &'static str),
}

const DEPS: &[Dep] = &[
    Dep {
        bin: "curl",
        packages: ("curl", "curl", "curl"),
    },
    Dep {
        bin: "wg-quick",
        packages: ("wireguard-tools", "wireguard-tools", "wireguard-tools"),
    },
    Dep {
        bin: "wg",
        packages: ("wireguard-tools", "wireguard-tools", "wireguard-tools"),
    },
    Dep {
        bin: "ping",
        packages: ("iputils", "iputils-ping", "iputils"),
    },
];

const SUDO: Dep = Dep {
    bin: "sudo",
    packages: ("sudo", "sudo", "sudo"),
};

enum Pm {
    Pacman,
    Apt,
    Dnf,
}

impl Pm {
    fn detect() -> Option<Self> {
        if on_path("pacman") {
            Some(Self::Pacman)
        } else if on_path("apt-get") {
            Some(Self::Apt)
        } else if on_path("dnf") {
            Some(Self::Dnf)
        } else {
            None
        }
    }

    fn package_for(&self, dep: &Dep) -> &'static str {
        match self {
            Self::Pacman => dep.packages.0,
            Self::Apt => dep.packages.1,
            Self::Dnf => dep.packages.2,
        }
    }

    fn install_argv(&self, packages: &[&str], as_root: bool) -> Vec<String> {
        let mut pkgs: Vec<&str> = packages.to_vec();
        pkgs.sort_unstable();
        pkgs.dedup();

        let mut cmd = Vec::new();
        if !as_root {
            cmd.push("sudo".into());
        }
        match self {
            Self::Pacman => {
                cmd.extend(["pacman".into(), "-S".into(), "--needed".into(), "--noconfirm".into()]);
            }
            Self::Apt => {
                cmd.extend(["apt-get".into(), "install".into(), "-y".into()]);
            }
            Self::Dnf => {
                cmd.extend(["dnf".into(), "install".into(), "-y".into()]);
            }
        }
        cmd.extend(pkgs.into_iter().map(str::to_string));
        cmd
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Pacman => "pacman",
            Self::Apt => "apt",
            Self::Dnf => "dnf",
        }
    }
}

fn on_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(bin).is_file()))
        .unwrap_or(false)
}

fn geteuid() -> u32 {
    unsafe { libc::geteuid() }
}

fn missing_deps() -> Vec<&'static Dep> {
    let mut missing: Vec<&'static Dep> = DEPS.iter().filter(|d| !on_path(d.bin)).collect();
    // Connect/disconnect needs root (setuid install / already root) or sudo.
    if geteuid() != 0 && !crate::utils::wg::saved_root() && !on_path(SUDO.bin) {
        missing.push(&SUDO);
    }
    missing
}

/// Check required tools; if any are missing, prompt to install and exit on failure.
pub fn ensure() -> color_eyre::Result<()> {
    let missing = missing_deps();
    if missing.is_empty() {
        return Ok(());
    }

    eprintln!("Missing required tools:");
    for d in &missing {
        eprintln!("  - {}", d.bin);
    }
    eprintln!();

    let Some(pm) = Pm::detect() else {
        eprintln!(
            "Could not detect a supported package manager (pacman/apt/dnf).\n\
             Install the tools above and re-run."
        );
        std::process::exit(1);
    };

    let as_root = geteuid() == 0;
    let needs_sudo = !as_root;
    let sudo_missing = missing.iter().any(|d| d.bin == "sudo");

    if needs_sudo && sudo_missing {
        // Cannot bootstrap sudo via sudo — print packages and bail.
        let packages: Vec<&str> = missing.iter().map(|d| pm.package_for(d)).collect();
        let mut pkgs = packages;
        pkgs.sort_unstable();
        pkgs.dedup();
        eprintln!(
            "`sudo` is missing and you are not root.\n\
             As root, install:\n  {} {}\n\
             Then re-run.",
            match pm {
                Pm::Pacman => "pacman -S --needed",
                Pm::Apt => "apt-get install -y",
                Pm::Dnf => "dnf install -y",
            },
            pkgs.join(" ")
        );
        std::process::exit(1);
    }

    let packages: Vec<&str> = missing.iter().map(|d| pm.package_for(d)).collect();
    let cmd = pm.install_argv(&packages, as_root);
    let cmd_display = cmd.join(" ");

    eprintln!("Install via {} with:\n  {cmd_display}\n", pm.label());
    eprint!("Install now? [Y/n] ");
    let _ = io::stderr().flush();

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let answer = answer.trim();
    if !answer.is_empty()
        && !answer.eq_ignore_ascii_case("y")
        && !answer.eq_ignore_ascii_case("yes")
    {
        eprintln!("Aborting — install the missing tools and re-run.");
        std::process::exit(1);
    }

    eprintln!("Running: {cmd_display}");
    let status = Command::new(&cmd[0])
        .args(&cmd[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| color_eyre::eyre::eyre!("failed to run installer: {e}"))?;

    if !status.success() {
        eprintln!("Install failed (exit {status}). Fix manually and re-run.");
        std::process::exit(1);
    }

    let still = missing_deps();
    if !still.is_empty() {
        let names: Vec<_> = still.iter().map(|d| d.bin).collect();
        eprintln!(
            "Still missing after install: {}. Fix manually and re-run.",
            names.join(", ")
        );
        std::process::exit(1);
    }

    eprintln!("All required tools are available.\n");
    Ok(())
}
