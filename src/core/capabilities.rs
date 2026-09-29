use serde::{Deserialize, Serialize};

use crate::policy::policy_constants::{
    FILE_PROTECTION_ENABLED,
    POLICY_USB_STORAGE_BLOCK,
    POLICY_MOBILE_DEVICE_BLOCK,
    POLICY_USB_DEVICE_MONITOR,
    POLICY_WEB_MONITOR_HISTORY,
    POLICY_WEB_URL_BLOCK,
    POLICY_WEB_PARTIAL_ACCESS,
    POLICY_APP_USAGE_MONITOR,
    POLICY_OCR_MONITOR,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyCapability {
    pub code: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub action: String,
    pub target: String,
    pub severity: String,
}

impl PolicyCapability {
    /// Returns all capabilities supported by the macOS agent.
    /// Notice: UEBA is explicitly omitted per system design.
    pub fn all_capabilities() -> Vec<Self> {
        [
            Self::usb_capabilities(),
            Self::web_proxy_capabilities(),
            Self::file_capabilities(),
            Self::app_usage_capabilities(),
            Self::ocr_capabilities(),
        ].concat()
    }

    /// External device capabilities (Standard USB, External SSD, SD Card, Mobile MTP)
    pub fn usb_capabilities() -> Vec<Self> {
        vec![
            PolicyCapability {
                code: POLICY_USB_STORAGE_BLOCK.to_string(),
                name: "External Device Storage Blocking".to_string(),
                description: "Block unauthorized Standard USB flash drives, External SSDs, and SD cards using DiskArbitration.".to_string(),
                category: "USB".to_string(),
                action: "BLOCK".to_string(),
                target: "EXTERNAL_STORAGE".to_string(),
                severity: "HIGH".to_string(),
            },
            PolicyCapability {
                code: POLICY_MOBILE_DEVICE_BLOCK.to_string(),
                name: "Mobile Device Blocking".to_string(),
                description: "Block mobile phones and tablets (MTP/PTP) from accessing and transferring files.".to_string(),
                category: "USB".to_string(),
                action: "BLOCK".to_string(),
                target: "MOBILE_DEVICES".to_string(),
                severity: "HIGH".to_string(),
            },
            PolicyCapability {
                code: POLICY_USB_DEVICE_MONITOR.to_string(),
                name: "External Device Connection Monitoring".to_string(),
                description: "Monitor and log all external device connection and disconnection events on macOS.".to_string(),
                category: "USB".to_string(),
                action: "MONITOR".to_string(),
                target: "USB_CONNECTIONS".to_string(),
                severity: "LOW".to_string(),
            },
        ]
    }

    /// Web filtering and partial access control
    pub fn web_proxy_capabilities() -> Vec<Self> {
        vec![
            PolicyCapability {
                code: POLICY_WEB_MONITOR_HISTORY.to_string(),
                name: "Web Activity Monitoring".to_string(),
                description: "Collect and report visited URLs and browsing duration via local proxy.".to_string(),
                category: "WEB".to_string(),
                action: "MONITOR".to_string(),
                target: "BROWSING_HISTORY".to_string(),
                severity: "MEDIUM".to_string(),
            },
            PolicyCapability {
                code: POLICY_WEB_URL_BLOCK.to_string(),
                name: "Block Websites by URL".to_string(),
                description: "Block access to forbidden websites using domain and URL rules.".to_string(),
                category: "WEB".to_string(),
                action: "BLOCK".to_string(),
                target: "URLS".to_string(),
                severity: "HIGH".to_string(),
            },
            PolicyCapability {
                code: POLICY_WEB_PARTIAL_ACCESS.to_string(),
                name: "Partial Access Control".to_string(),
                description: "Control upload and download dialogs and actions on confidential or cloud sites.".to_string(),
                category: "WEB".to_string(),
                action: "CONTROL".to_string(),
                target: "UPLOAD_DOWNLOAD_DIALOGS".to_string(),
                severity: "MEDIUM".to_string(),
            },
        ]
    }

    /// Application usage tracking
    pub fn app_usage_capabilities() -> Vec<Self> {
        vec![
            PolicyCapability {
                code: POLICY_APP_USAGE_MONITOR.to_string(),
                name: "Monitor App Usage".to_string(),
                description: "Track foreground applications, active duration, and category breakdown using NSWorkspace.".to_string(),
                category: "APP_USAGE".to_string(),
                action: "MONITOR".to_string(),
                target: "APPLICATIONS".to_string(),
                severity: "LOW".to_string(),
            },
        ]
    }

    /// Filesystem protection
    pub fn file_capabilities() -> Vec<Self> {
        vec![
            PolicyCapability {
                code: FILE_PROTECTION_ENABLED.to_string(),
                name: "File Protection System".to_string(),
                description: "Complete macOS filesystem protection with Endpoint Security kernel-level enforcement.".to_string(),
                category: "FILE".to_string(),
                action: "ENFORCE".to_string(),
                target: "FILESYSTEM".to_string(),
                severity: "HIGH".to_string(),
            },
        ]
    }

    /// OCR and Screen Monitoring
    pub fn ocr_capabilities() -> Vec<Self> {
        vec![
            PolicyCapability {
                code: POLICY_OCR_MONITOR.to_string(),
                name: "OCR Screen Monitoring".to_string(),
                description: "Monitor screen content for sensitive data violations using OCR and LLM threat assessment.".to_string(),
                category: "SECURITY_MONITOR".to_string(),
                action: "MONITOR".to_string(),
                target: "SCREEN_CONTENT".to_string(),
                severity: "MEDIUM".to_string(),
            },
        ]
    }
}
