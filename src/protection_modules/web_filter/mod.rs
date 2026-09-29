//! protection_modules/web_filter/mod.rs
//!
//! Web Filtering and Partial Access for macOS.

pub mod mitm_proxy;
pub mod partial_access;

pub use mitm_proxy::WebProtectionModule;
