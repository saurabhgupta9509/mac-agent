//! protection_modules/file_protection/mod.rs
//!
//! ╔══════════════════════════════════════════════════════════════════╗
//! ║         macOS File Protection — Endpoint Security Framework     ║
//! ║                                                                  ║
//! ║  HOW IT WORKS:                                                   ║
//! ║  ┌───────────────────────────────────────────────────────┐      ║
//! ║  │  1. Agent creates an ES (Endpoint Security) client    │      ║
//! ║  │  2. We subscribe to AUTH events:                      │      ║
//! ║  │     - ES_EVENT_TYPE_AUTH_OPEN    (file open/read)     │      ║
//! ║  │     - ES_EVENT_TYPE_AUTH_CREATE  (file create)        │      ║
//! ║  │     - ES_EVENT_TYPE_AUTH_WRITE   (file modify)        │      ║
//! ║  │     - ES_EVENT_TYPE_AUTH_RENAME  (file rename/move)   │      ║
//! ║  │     - ES_EVENT_TYPE_AUTH_UNLINK  (file delete)        │      ║
//! ║  │  3. For each AUTH event:                              │      ║
//! ║  │     a. Get path from event                            │      ║
//! ║  │     b. Check path against policy rules               │      ║
//! ║  │     c. ALLOW → es_respond_auth_result(ALLOW)         │      ║
//! ║  │        BLOCK → es_respond_auth_result(DENY)           │      ║
//! ║  │     d. Send alert to server if blocked               │      ║
//! ║  └───────────────────────────────────────────────────────┘      ║
//! ║                                                                  ║
//! ║  Requires entitlement:                                           ║
//! ║    com.apple.security.endpoint-security.client                   ║
//! ║  Must run as root (LaunchDaemon).                                ║
//! ║                                                                  ║
//! ║  Windows equivalent:                                             ║
//! ║    Kernel minifilter driver (FLTMGR) + IRP_MJ_CREATE intercept ║
//! ╚══════════════════════════════════════════════════════════════════╝

pub mod file_protection_module;
pub mod policy_engine;
pub mod path_resolver;
pub mod file_browser;

pub use file_protection_module::FileProtectionModule;
