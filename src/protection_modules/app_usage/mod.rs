//! protection_modules/app_usage/mod.rs + app_usage_module.rs
//!
//! ╔══════════════════════════════════════════════════════════════════╗
//! ║         macOS App Usage Monitoring                              ║
//! ║                                                                  ║
//! ║  HOW IT WORKS:                                                   ║
//! ║  ┌───────────────────────────────────────────────────────┐      ║
//! ║  │  Every 1 second:                                      │      ║
//! ║  │    NSWorkspace.shared.frontmostApplication            │      ║
//! ║  │        → returns current active app name              │      ║
//! ║  │                                                       │      ║
//! ║  │  Track duration per app:                             │      ║
//! ║  │    "Chrome" → 450 seconds                            │      ║
//! ║  │    "Slack"  → 120 seconds                            │      ║
//! ║  │                                                       │      ║
//! ║  │  Idle detection:                                      │      ║
//! ║  │    CGEventSourceSecondsSinceLastEventType()           │      ║
//! ║  │    > 60s → mark as "Idle"                            │      ║
//! ║  │                                                       │      ║
//! ║  │  Every 5 minutes:                                     │      ║
//! ║  │    POST /api/agent/app-usage with JSON payload        │      ║
//! ║  └───────────────────────────────────────────────────────┘      ║
//! ║                                                                  ║
//! ║  Window title reading:                                           ║
//! ║    CGWindowListCopyWindowInfo → gets active window title        ║
//! ║                                                                  ║
//! ║  Windows equivalent:                                             ║
//! ║    GetForegroundWindow() → GetWindowThreadProcessId()           ║
//! ║    → sysinfo::Process::name() + UIAutomation title             ║
//! ╚══════════════════════════════════════════════════════════════════╝

pub mod app_usage_module;
pub use app_usage_module::AppUsageModule;
