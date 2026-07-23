# vpn

A terminal UI for managing Surfshark and ProtonVPN WireGuard connections, built
with [ratatui](https://ratatui.rs).

Press **Alt+Space** to switch providers. Each provider keeps its own session in
`~/.config/vpn/` (`surfshark.json`, `proton.json`, `prefs.json`).

## Installation

### Quick install (Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/protibimbok/vpn-tui/master/scripts/install.sh | bash
```

This downloads the release binary and installs it **setuid-root** to
`/usr/local/bin/vpn` (sudo required once). The app drops privileges at
startup and re-elevates only `wg`/`wg-quick` child processes.

### apt (Debian / Ubuntu)

```bash
# One-time: install the signing key
curl -fsSL \
  https://github.com/protibimbok/pkg-dist/raw/master/public.gpg \
  | sudo gpg --dearmor \
  -o /usr/share/keyrings/protibimbok.gpg

# One-time: add the repository
echo "deb [signed-by=/usr/share/keyrings/protibimbok.gpg] \
  https://protibimbok.github.io/pkg-dist/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/protibimbok.list

sudo apt update
sudo apt install vpn
```

### pacman / AUR (Arch Linux)

```bash
yay -S vpn-bin
# or
paru -S vpn-bin
```

### rpm (Fedora / RHEL / openSUSE)

Download the `.rpm` from the [latest release](https://github.com/protibimbok/vpn-tui/releases/latest):

```bash
sudo rpm -i vpn_*_linux_amd64.rpm
```

### Alpine Linux

```bash
# Download the .apk from the latest release, then:
sudo apk add --allow-untrusted vpn_*_linux_amd64.apk
```

### Build from source

```sh
cargo build --release
sudo install -o root -g root -m 4755 target/release/vpn /usr/local/bin/vpn
vpn
```

## Requirements

Runtime tools (the TUI shells out to these):

| Tool | Package (typical) | Used for |
|------|-------------------|----------|
| `wg`, `wg-quick` | `wireguard-tools` | Bring the tunnel up/down and query status |
| `curl` | `curl` | Provider API calls |
| `ping` | `iputils` / `iputils-ping` | Server latency checks |
| `resolvconf` | see below | Apply the config `DNS =` lines via `wg-quick` |

**DNS / `resolvconf`:** `wg-quick` always calls `resolvconf` when a config has `DNS =`. On systems that use **systemd-resolved**, install `systemd-resolvconf` (Arch: `pacman -S systemd-resolvconf`) so `/usr/bin/resolvconf` is the `resolvectl` shim. Do **not** use `openresolv` against a systemd-managed `/etc/resolv.conf` — that produces `resolvconf: signature mismatch` and tears the interface down.

**Privileges:** Packages and the install script set the setuid bit automatically. Building from source requires the manual `install -m 4755` step above. Alternatives: a passwordless sudo rule for `wg-quick` (no handshake age in the status bar), or running as root.

## Release setup (for maintainers)

Homebrew and apt distribution are managed centrally in
[protibimbok/pkg-dist](https://github.com/protibimbok/pkg-dist). This repo
builds binaries and publishes GitHub Releases.

### Required GitHub secrets (this repo)

| Secret | Purpose | Required |
|--------|---------|----------|
| `GITHUB_TOKEN` | Create GitHub Releases | Auto-provided |
| `PKG_DIST_TOKEN` | Trigger pkg-dist apt update after release | For apt |
| `AUR_KEY` | SSH private key for AUR updates | For AUR |

### Creating a release

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow cross-compiles with `cargo zigbuild`, publishes tarballs and
Linux packages to GitHub Releases, updates AUR (`vpn-bin`), and notifies
pkg-dist to refresh the apt repository.
