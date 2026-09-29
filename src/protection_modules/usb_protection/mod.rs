//! protection_modules/usb_protection/mod.rs
//!
//! USB Protection for macOS.
//! Replaces Windows WDM USB filter driver + HID blocking.
//!
//! macOS approach:
//!   1. DiskArbitration  → detect/deny USB drive mounts
//!   2. IOKit            → detect/block MTP/PTP mobile devices
//!   3. Endpoint Security → monitor file copy to mounted USB

pub mod usb_guard;
pub mod mobile_block;
pub mod usb_monitor;

pub use usb_guard::UsbGuard;
