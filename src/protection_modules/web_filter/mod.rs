//! protection_modules/web_filter/mod.rs
//!
//! Web Filtering for macOS.
//! Replaces WFP + WinDivert with Network Extension + MITM proxy.
//!
//! Architecture:
//!   System Extension (NEFilterDataProvider)
//!     ├── Intercepts all TCP/UDP flows
//!     └── Routes suspicious flows through MITM proxy for inspection
//!
//!   MITM Proxy (same Rust code as Linux agent)
//!     ├── TLS termination (rustls)
//!     ├── URL classification
//!     └── Content scanning

pub mod network_extension;
pub mod mitm_proxy;
pub mod tls;
pub mod web_filter_phase2;
pub mod web_filter_phase3;
