//! protection_modules/usb_protection/usb_monitor.rs
//! USB event telemetry — reports plug/unplug events to server.

use crate::core::file_logger::FileLogger;

pub struct UsbMonitor;

impl UsbMonitor {
    pub fn start() {
        FileLogger::info("[UsbMonitor] USB event telemetry started (placeholder).");
        // TODO: Subscribe to IOKit USB notifications and report to server
    }
}
