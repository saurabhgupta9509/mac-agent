//! policy/policy_constants.rs
//!
//! Exactly matches Windows/Linux policy_constants.rs
//! Same constants — server sends same JSON keys to all platforms.

// External Device Protection
pub const POLICY_USB_STORAGE_BLOCK: &str  = "USB_STORAGE_BLOCK";
pub const POLICY_MOBILE_DEVICE_BLOCK: &str = "MOBILE_DEVICE_BLOCK";
pub const POLICY_USB_DEVICE_MONITOR: &str  = "USB_DEVICE_MONITOR";

// Web Protection
pub const POLICY_WEB_MONITOR_HISTORY: &str = "WEB_MONITOR_HISTORY";
pub const POLICY_WEB_URL_BLOCK: &str       = "WEB_URL_BLOCK";
pub const POLICY_WEB_PARTIAL_ACCESS: &str  = "WEB_PARTIAL_ACCESS";

// File Protection
pub const FILE_PROTECTION_ENABLED: &str    = "POLICY_FILE_PROTECTION";

// App Usage Monitoring
pub const POLICY_APP_USAGE_MONITOR: &str   = "APP_USAGE_MONITOR";

// Security Monitor / OCR
pub const POLICY_OCR_MONITOR: &str         = "POLICY_OCR_MONITOR";

// Network DNS Block
pub const POLICY_NETWORK_DNS_BLOCK: &str   = "NETWORK_DNS_BLOCK";
