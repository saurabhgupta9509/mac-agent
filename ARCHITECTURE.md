# 🍎 Mac DLP Agent — Architecture & Migration Plan

> **Source**: Converted from `linux-dlp-agent` (Windows Rust agent)  
> **Target**: macOS 12+ (Monterey and above), Apple Silicon (M1/M2/M3) + Intel  
> **Language**: Rust (same codebase philosophy)  
> **Build Tool**: Cargo + `cargo-bundle` / `pkgbuild`

---

## 📐 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          mac-dlp-agent                                  │
│                                                                         │
│  ┌───────────────┐   ┌───────────────┐   ┌──────────────────────────┐  │
│  │  LaunchDaemon │   │  LaunchAgent  │   │   Chrome/Safari Extension│  │
│  │  (root level) │   │ (user session)│   │   (Partial Access / Proxy)│  │
│  └──────┬────────┘   └──────┬────────┘   └───────────┬──────────────┘  │
│         │                   │                         │                 │
│  ┌──────▼───────────────────▼─────────────────────────▼──────────────┐ │
│  │                     Core Agent (Tokio Runtime)                     │ │
│  │                                                                    │ │
│  │  ┌─────────────┐  ┌──────────────┐  ┌───────────────────────────┐ │ │
│  │  │PolicyEngine │  │ Communicator │  │    ProtectionModule Trait  │ │ │
│  │  │(server sync)│  │ (REST+WS)    │  │  (execute, get_name, etc.) │ │ │
│  │  └─────────────┘  └──────────────┘  └───────────────────────────┘ │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                                                         │
│  ┌─────────────────────────── Protection Modules ─────────────────────┐ │
│  │                                                                    │ │
│  │  USB           Web Filter      File          Security Monitor      │ │
│  │  Protection    (MITM Proxy)    Protection    (OCR + LLM)           │ │
│  │                                                                    │ │
│  │  App Usage     Partial Access  Print         Clipboard Monitor     │ │
│  │  Monitor       (JS Inject)     Monitor       (Screen Capture)      │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                                                         │
│  ┌─────────────────────── Telemetry / UEBA ───────────────────────────┐ │
│  │  app_tracker  │  file_monitor  │  print_monitor  │  clipboard_mon  │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                                                         │
│  ┌─────────────────────── macOS Platform Layer ───────────────────────┐ │
│  │  endpoint_security  │  network_extension  │  system_extension      │ │
│  │  (ES Framework)     │  (NEFilterDataProvider)  │  (DriverKit)       │ │
│  └────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 🗂️ Folder Structure

```
mac-dlp-agent/
├── Cargo.toml                        # Workspace manifest
├── Cargo.lock
├── ARCHITECTURE.md                   # This file
├── README.md
├── build.rs                          # Build script (codesign, entitlements)
│
├── src/
│   ├── main.rs                       # Entry: LaunchDaemon mode OR --ui-helper
│   ├── lib.rs                        # Library root (shared types)
│   │
│   ├── core/
│   │   ├── mod.rs
│   │   ├── agent_core.rs             # Main agent loop (≈ Windows AgentCore)
│   │   ├── service_runner.rs         # LaunchDaemon lifecycle manager
│   │   ├── capabilities.rs           # Policy capability declarations
│   │   ├── config.rs                 # Config loading (plist/JSON)
│   │   ├── credential_store.rs       # macOS Keychain integration
│   │   ├── file_logger.rs            # Structured logger → /var/log/dlp-agent/
│   │   ├── network_utils.rs          # IP/MAC utilities
│   │   ├── permission_utils.rs       # SIP-safe permission checks
│   │   ├── self_destruct.rs          # Remote wipe / self-destruct
│   │   └── ueba_config.rs            # UEBA policy config
│   │
│   ├── policy/
│   │   ├── mod.rs
│   │   ├── policy_engine.rs          # PolicyEngine (fetch, evaluate, cache)
│   │   ├── policy_constants.rs       # Policy code strings
│   │   └── policy_store.rs           # Local policy cache
│   │
│   ├── protection_modules/
│   │   ├── mod.rs                    # ProtectionModule trait
│   │   ├── dto.rs                    # Shared DTOs (events, alerts)
│   │   │
│   │   ├── usb_protection/
│   │   │   ├── mod.rs
│   │   │   ├── usb_guard.rs          # DiskArbitration callbacks (DARegisterDiskAppearedCallback)
│   │   │   ├── mobile_block.rs       # MTP/PTP blocking via IOKit
│   │   │   └── usb_monitor.rs        # USB event telemetry
│   │   │
│   │   ├── web_filter/
│   │   │   ├── mod.rs
│   │   │   ├── network_extension.rs  # NEFilterDataProvider (System Extension)
│   │   │   ├── mitm_proxy.rs         # MITM proxy (HTTP/HTTPS intercept)
│   │   │   ├── tls.rs                # TLS termination / cert pinning
│   │   │   ├── web_filter_phase2.rs  # URL classification
│   │   │   └── web_filter_phase3.rs  # Content scanning
│   │   │
│   │   ├── file_protection/
│   │   │   ├── mod.rs
│   │   │   ├── endpoint_security.rs  # ES_EVENT_TYPE_AUTH_OPEN/WRITE (ES Framework)
│   │   │   ├── policy_engine.rs      # File policy evaluation
│   │   │   ├── path_resolver.rs      # HFS+ / APFS path normalization
│   │   │   └── file_browser.rs       # Remote file browsing API
│   │   │
│   │   ├── partial_access/
│   │   │   ├── mod.rs
│   │   │   ├── proxy.rs              # Partial access proxy logic
│   │   │   ├── configurator.rs       # Safari/Chrome proxy setup
│   │   │   ├── sni_parser.rs         # SNI parsing (same as Linux)
│   │   │   ├── tls.rs                # TLS for partial access
│   │   │   └── extension/            # Chrome/Safari extension (JS)
│   │   │       ├── manifest.json
│   │   │       ├── background.js
│   │   │       ├── content.js
│   │   │       └── inject.js
│   │   │
│   │   ├── security_monitor/
│   │   │   ├── mod.rs
│   │   │   ├── monitor.rs            # Screen capture (CGDisplayCreateImage)
│   │   │   ├── ocr.rs                # OCR (Vision framework / Tesseract)
│   │   │   ├── image_processing.rs   # Image preprocessing
│   │   │   ├── llm_threat_assessor.rs # LLM-based threat analysis
│   │   │   ├── rule_engine.rs        # Rule matching engine
│   │   │   └── entity_engine.rs      # Named entity detection
│   │   │
│   │   └── app_usage/
│   │       ├── mod.rs
│   │       └── app_usage.rs          # NSWorkspace notifications (active app)
│   │
│   ├── proxy/
│   │   ├── mod.rs
│   │   ├── configurator.rs           # PAC file / proxy env vars for macOS
│   │   ├── mitm.rs                   # MITM proxy core
│   │   ├── ca_manager.rs             # Certificate Authority manager
│   │   └── tls.rs                    # TLS stack (rustls)
│   │
│   ├── telemetry/
│   │   ├── mod.rs                    # start_telemetry_collectors()
│   │   ├── app_tracker/
│   │   │   ├── mod.rs
│   │   │   └── app_tracker.rs        # Active app via NSWorkspace / CGWindow
│   │   ├── clipboard_monitor/
│   │   │   ├── mod.rs
│   │   │   └── clipboard_screen.rs   # NSPasteboard monitoring + screenshots
│   │   ├── file_monitor/
│   │   │   ├── mod.rs
│   │   │   └── file_usb_monitor.rs   # FSEvents API (macOS file change events)
│   │   └── print_monitor/
│   │       ├── mod.rs
│   │       └── print_monitor.rs      # CUPS print job monitoring
│   │
│   ├── transport/
│   │   ├── mod.rs
│   │   └── dispatcher.rs             # Event dispatcher (same as Linux)
│   │
│   ├── aggregator/
│   │   ├── mod.rs
│   │   └── tumbling_window.rs        # UEBA tumbling window aggregator
│   │
│   ├── websocket/
│   │   ├── mod.rs
│   │   └── agent_ws.rs               # WebSocket client (server comms)
│   │
│   ├── platform/
│   │   └── macos/
│   │       ├── mod.rs
│   │       ├── endpoint_security.rs  # Raw ES Framework bindings (unsafe)
│   │       ├── disk_arbitration.rs   # DiskArbitration bindings
│   │       ├── network_extension.rs  # NEFilterDataProvider glue
│   │       ├── keychain.rs           # Security.framework Keychain API
│   │       └── authorization.rs      # AuthorizationServices (privilege escalation)
│   │
│   └── installer/
│       ├── mod.rs
│       └── installer.rs             # First-run setup, SysExt activation
│
├── platform/
│   ├── macos/
│   │   ├── entitlements.plist        # com.apple.security.system-extension, etc.
│   │   └── Info.plist                # Bundle metadata
│
├── packaging/
│   ├── plist/
│   │   ├── com.dlpagent.daemon.plist  # LaunchDaemon (root)
│   │   └── com.dlpagent.agent.plist   # LaunchAgent (user)
│   ├── scripts/
│   │   ├── preinstall.sh             # Pre-install script for .pkg
│   │   ├── postinstall.sh            # Post-install: load daemon, activate extension
│   │   └── uninstall.sh              # Full removal script
│   └── pkg/
│       └── build_pkg.sh              # pkgbuild + productbuild invocation
│
├── chrome_extension/                 # Shared Chrome extension
│   ├── manifest.json
│   ├── background.js
│   ├── content.js
│   └── inject.js
│
├── tessdata/                         # OCR language data
│   └── eng.traineddata
│
├── certificates/                     # Root CA certs
│   └── dlp_root_ca.crt
│
└── docs/
    ├── MIGRATION_GUIDE.md
    ├── MACOS_PERMISSIONS.md
    └── BUILD.md
```

---

## 🔄 Feature-by-Feature Migration Map

| Linux/Windows Feature | File(s) | macOS Equivalent | macOS API |
|---|---|---|---|
| **Windows Service** | `main.rs`, `service_runner.rs` | **LaunchDaemon** | `launchctl`, plist |
| **WFP Driver** (network filter) | `wfp_driver/` | **Network Extension** | `NEFilterDataProvider` |
| **WinDivert** (packet capture) | `windivert` crate | **Network Extension** | `NEFilterDataProvider` |
| **ETW Network Monitor** | `etw_network.rs` | **Network Extension / tcpdump** | `NEPacketTunnelProvider` |
| **USB Protection** (HID block) | `usb_protection.rs` | **DiskArbitration + IOKit** | `DARegisterDiskAppearedCallback`, `IOUSBDevice` |
| **File Protection** (kernel events) | `file_protection/kernel/` | **Endpoint Security** | `ESClient`, `es_subscribe()` |
| **App Monitor** (foreground window) | `app_tracker.rs` | **NSWorkspace** | `NSWorkspace.shared.frontmostApplication` |
| **Web MITM Proxy** | `mitm_proxy.rs`, `proxy/` | **MITM Proxy** (same Rust) | `rustls` + system proxy settings |
| **Partial Access** (Chrome ext) | `partial_access/` | **Same** (Chrome ext) | Chrome extension API |
| **Print Monitor** | `print_monitor.rs` | **CUPS Monitor** | CUPS API / `lpstat` / IPP |
| **Clipboard Monitor** | `clipboard_screen.rs` | **NSPasteboard** | `NSPasteboard.changeCount` |
| **Screen Capture** (OCR) | `security_monitor/` | **CGDisplayCreateImage** | Core Graphics |
| **OCR** | Tesseract / paddle-ocr | **Apple Vision + Tesseract** | `VNRecognizeTextRequest` |
| **Credential Store** | `credential_store.rs` | **macOS Keychain** | `Security.framework` |
| **Permissions Hardening** | `permission_utils.rs` | **SIP + cs_ops** | `codesign`, entitlements |
| **Self Destruct** | `self_destruct.rs` | **Same** (file removal) | `std::fs::remove_file` |
| **Event Log Parser** | `eventlog_parser.rs` | **Unified Log (OSLog)** | `log stream`, OSLog API |
| **Windows Registry** | `winreg` | **UserDefaults / plist** | `UserDefaults`, plist files |
| **Admin Elevation** | `ShellExecuteExW + runas` | **AuthorizationServices** | `AuthorizationExecuteWithPrivileges` |
| **DACL Hardening** | `permission_utils.rs` | **POSIX DAC + SIP** | `chmod`, `chown`, entitlements |
| **NSIS Installer** | `installer.nsi` | **pkg + productbuild** | `pkgbuild`, `productbuild` |
| **UI Helper** (desktop) | `dlp-ui.exe` | **macOS App Bundle** | `.app`, LaunchAgent |

---

## 🍎 macOS-Specific Architecture Decisions

### 1. LaunchDaemon vs LaunchAgent
```
LaunchDaemon (root, /Library/LaunchDaemons/)
  └── dlp-agent (core service) — runs as root
      ├── Loads System Extension (Network Filter, Endpoint Security)
      ├── Manages policy sync
      └── Spawns LaunchAgent for user-session work

LaunchAgent (user, ~/Library/LaunchAgents/ or /Library/LaunchAgents/)
  └── dlp-agent --ui-helper — runs as logged-in user
      ├── App usage monitoring (NSWorkspace)
      ├── Clipboard monitoring (NSPasteboard)
      └── Screen capture for OCR (Core Graphics)
```

### 2. System Extensions (Replaces Kernel Drivers)
macOS 10.15+ deprecates kernel extensions (KEXTs). We use **System Extensions**:
- **Network Extension** → replaces WFP + WinDivert
- **Endpoint Security** → replaces file-system kernel events

```
┌─────────────────────────────────────────────────────┐
│              System Extension Bundle                 │
│  com.yourcompany.dlp-agent.network-extension         │
│                                                     │
│  NEFilterDataProvider                               │
│    ├── handleNewFlow()  ← intercept all TCP/UDP     │
│    ├── handleRulesChanged()                         │
│    └── verdict: allow / drop / remediateExternally  │
└─────────────────────────────────────────────────────┘
```

### 3. Endpoint Security (File Protection)
```rust
// src/platform/macos/endpoint_security.rs
// ES Framework replaces the Windows kernel bridge
es_subscribe(client, &[
    ES_EVENT_TYPE_AUTH_OPEN,
    ES_EVENT_TYPE_AUTH_CREATE,
    ES_EVENT_TYPE_AUTH_WRITE,
    ES_EVENT_TYPE_AUTH_RENAME,
    ES_EVENT_TYPE_AUTH_UNLINK,
]);
```

### 4. USB Protection (DiskArbitration)
```rust
// src/platform/macos/disk_arbitration.rs
// Replaces Windows WDM USB filter driver
DARegisterDiskAppearedCallback(session, None, disk_appeared_cb, ctx);
DARegisterDiskDisappearedCallback(session, None, disk_disappeared_cb, ctx);
// Deny mount: DADissenterCreate(kDAReturnNotPermitted, ...)
```

### 5. Proxy Configuration (System Proxy)
```rust
// src/proxy/configurator.rs  
// macOS uses scutil / networksetup instead of Windows WFP
// Set system proxy via networksetup CLI:
//   networksetup -setwebproxy "Wi-Fi" 127.0.0.1 8080
//   networksetup -setsecurewebproxy "Wi-Fi" 127.0.0.1 8080
```

### 6. Credential Storage (Keychain)
```rust
// src/platform/macos/keychain.rs
// Replaces Windows Credential Manager / winreg
// Use Security.framework:
//   SecItemAdd, SecItemCopyMatching, SecItemDelete
```

---

## 📦 Packaging Plan

### `.pkg` Installer Flow
```
productbuild
  ├── component: dlp-agent (LaunchDaemon)
  ├── component: dlp-ui.app (LaunchAgent)
  ├── component: com.dlpagent.network-extension (System Extension)
  ├── preinstall.sh  → check macOS version, SIP status
  └── postinstall.sh → launchctl bootstrap, activate system extension
```

### Entitlements Required
```xml
<!-- platform/macos/entitlements.plist -->
<key>com.apple.security.system-extension</key>         <!-- System Extension -->
<key>com.apple.security.network.client</key>            <!-- Outbound network -->
<key>com.apple.security.network.server</key>            <!-- Local MITM proxy -->
<key>com.apple.security.endpoint-security.client</key>  <!-- ES Framework -->
<key>com.apple.security.application-groups</key>        <!-- IPC with daemon -->
<key>com.apple.security.temporary-exception.files.absolute-path.read-write</key>
```

---

## 🛠️ Build Pipeline

```bash
# Build for Apple Silicon (arm64)
cargo build --release --target aarch64-apple-darwin

# Build for Intel (x86_64)
cargo build --release --target x86_64-apple-darwin

# Universal Binary
lipo -create \
  target/aarch64-apple-darwin/release/dlp-agent \
  target/x86_64-apple-darwin/release/dlp-agent \
  -output dlp-agent-universal

# Code Sign (required for System Extensions)
codesign --force --deep --sign "Developer ID Application: YourOrg" \
  --entitlements platform/macos/entitlements.plist \
  dlp-agent-universal

# Package
pkgbuild --root staging/ \
  --identifier com.dlpagent.agent \
  --version 1.0.0 \
  --scripts packaging/scripts/ \
  dlp-agent.pkg
```

---

## 🏗️ Implementation Phases

### Phase 1 — Foundation (Week 1–2)
- [ ] `Cargo.toml` setup (remove Windows deps, add macOS crates)
- [ ] `main.rs` → LaunchDaemon entry point
- [ ] `core/service_runner.rs` → launchctl lifecycle
- [ ] `core/credential_store.rs` → Keychain integration
- [ ] `core/file_logger.rs` → `/var/log/dlp-agent/`
- [ ] `platform/macos/authorization.rs` → privilege elevation
- [ ] Basic `communication.rs` + `websocket/` (cross-platform, no changes needed)

### Phase 2 — Protection Modules (Week 3–5)
- [ ] `protection_modules/usb_protection/` → DiskArbitration
- [ ] `platform/macos/endpoint_security.rs` → ES Framework
- [ ] `protection_modules/file_protection/` → ES-based file events
- [ ] `proxy/` → MITM proxy (largely same as Linux)
- [ ] `proxy/configurator.rs` → `networksetup` CLI

### Phase 3 — Network & Web Filter (Week 6–8)
- [ ] `platform/macos/network_extension.rs` → NEFilterDataProvider
- [ ] `protection_modules/web_filter/network_extension.rs`
- [ ] `protection_modules/partial_access/` → Same Chrome extension
- [ ] DNS blocking via Network Extension

### Phase 4 — Telemetry / UEBA (Week 9–10)
- [ ] `telemetry/app_tracker/` → NSWorkspace
- [ ] `telemetry/clipboard_monitor/` → NSPasteboard
- [ ] `telemetry/file_monitor/` → FSEvents
- [ ] `telemetry/print_monitor/` → CUPS
- [ ] `aggregator/tumbling_window.rs` (unchanged)

### Phase 5 — Security Monitor (Week 11–12)
- [ ] `protection_modules/security_monitor/monitor.rs` → CGDisplayCreateImage
- [ ] `protection_modules/security_monitor/ocr.rs` → VNRecognizeTextRequest
- [ ] LLM Threat Assessor (unchanged, REST call)
- [ ] Rule engine + entity engine (unchanged)

### Phase 6 — Packaging & Signing (Week 13–14)
- [ ] entitlements.plist
- [ ] LaunchDaemon + LaunchAgent plists
- [ ] `packaging/scripts/postinstall.sh`
- [ ] `packaging/pkg/build_pkg.sh`
- [ ] Code signing + notarization workflow
- [ ] System Extension activation flow

---

## 🔑 Key macOS Crates

| Crate | Purpose | Replaces |
|---|---|---|
| `security-framework` | Keychain, TLS, certs | `winreg`, Credential Manager |
| `core-foundation` | CF types bridge | Windows COM |
| `objc2` | Objective-C bridge | - |
| `objc2-app-kit` | NSWorkspace, NSPasteboard | `uiautomation` |
| `objc2-vision` | VNRecognizeTextRequest (OCR) | Tesseract (fallback) |
| `core-graphics` | CGDisplayCreateImage | `screenshots` crate |
| `sysinfo` | Process info | `sysinfo` (same) |
| `tokio` | Async runtime | Same |
| `rustls` | TLS | Same |
| `axum` | HTTP server | Same |
| `notify` | File system events | `notify` (wraps FSEvents) |
| `cups-sys` | CUPS print monitor | `print_monitor.rs` |

---

## ⚠️ Critical macOS Differences

| Topic | Windows | macOS |
|---|---|---|
| **Kernel Drivers** | WFP / WDM drivers | ❌ No KEXTs → System Extensions |
| **Registry** | `HKLM\...\DLPAgent` | plist files / UserDefaults |
| **Service Control** | SCM (`sc.exe`) | launchctl / plist |
| **Admin Elevation** | `runas` / UAC | `AuthorizationServices` |
| **ETW** | ETW (Windows only) | Unified Logging (OSLog) |
| **WinDivert** | WinDivert driver | Network Extension |
| **UIAutomation** | `uiautomation` crate | `Accessibility` framework |
| **Process Protection** | DACL hardening | SIP + codesign |
| **App Sandbox** | Not applicable | Partially applies to .app |
| **Notarization** | Code Signing | Code Sign + Notarize (Apple) |

---

## 📋 Dependencies to Remove (Windows-only)

```toml
# REMOVE these from Cargo.toml:
windows-sys = ...
windows = ...
windows-service = ...
winreg = ...
windivert = ...
uiautomation = ...
etherparse = ...   # (WinDivert packet parsing)
leptonica-sys = ...  # (Tesseract Windows)
```

## 📋 Dependencies to Add (macOS)

```toml
[target.'cfg(target_os = "macos")'.dependencies]
security-framework = "2"
core-foundation = "0.10"
objc2 = "0.5"
objc2-app-kit = "0.2"
objc2-vision = "0.2"
core-graphics = "0.23"
cups-sys = "0.1"
block2 = "0.5"
dispatch = "0.2"
```
