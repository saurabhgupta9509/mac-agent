# 🍎 Mac DLP Agent

> macOS port of the Linux/Windows DLP Agent — same protection features, native macOS implementation.

## Requirements

- macOS 12.0+ (Monterey)
- Apple Silicon (arm64) or Intel (x86_64)
- Apple Developer account (for System Extensions + Notarization)
- Xcode Command Line Tools

## Features

| Feature | macOS Implementation |
|---|---|
| **Network Filtering** | Network Extension (NEFilterDataProvider) |
| **File Protection** | Endpoint Security Framework |
| **USB Blocking** | DiskArbitration + IOKit |
| **Web MITM Proxy** | Rust MITM proxy (same as Linux) |
| **Partial Access** | Chrome/Safari extension (same) |
| **App Monitoring** | NSWorkspace |
| **Clipboard Monitor** | NSPasteboard |
| **Screen OCR** | Core Graphics + Apple Vision |
| **Print Monitor** | CUPS API |
| **Credential Store** | macOS Keychain |
| **Service Management** | LaunchDaemon + LaunchAgent |
| **Installer** | .pkg (pkgbuild + productbuild) |

## Build

```bash
# Install Rust targets
rustup target add aarch64-apple-darwin x86_64-apple-darwin

# Build (on macOS)
cargo build --release --target aarch64-apple-darwin  # Apple Silicon
cargo build --release --target x86_64-apple-darwin   # Intel

# Build installer
bash packaging/pkg/build_pkg.sh
```

## Architecture

See [ARCHITECTURE.md](./ARCHITECTURE.md) for the full migration map, phase plan, and macOS API details.

## Directory Structure

```
mac-dlp-agent/
├── src/
│   ├── core/               # Agent core, service runner, credentials
│   ├── protection_modules/ # USB, Web Filter, File, Security Monitor
│   ├── telemetry/          # App tracker, clipboard, file, print
│   ├── proxy/              # MITM proxy (shared with Linux)
│   ├── platform/macos/     # macOS-specific bindings
│   └── installer/          # First-run setup
├── packaging/
│   ├── plist/              # LaunchDaemon + LaunchAgent plists
│   ├── scripts/            # pre/postinstall shell scripts
│   └── pkg/                # build_pkg.sh
├── platform/macos/
│   └── entitlements.plist  # Required Apple entitlements
└── ARCHITECTURE.md         # Full migration plan
```
