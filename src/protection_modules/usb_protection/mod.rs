//! protection_modules/usb_protection/mod.rs
//!
//! USB and External Device Protection for macOS using DiskArbitration.

pub mod usb_guard;
pub mod mobile_block;
pub mod usb_monitor;

pub use usb_guard::UsbProtectionModule;
