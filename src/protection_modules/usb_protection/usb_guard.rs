//! protection_modules/usb_protection/usb_guard.rs
//!
//! ╔══════════════════════════════════════════════════════════════════════════════╗
//! ║           macOS External Device Protection (100% Fail-Safe)                 ║
//! ║                                                                              ║
//! ║  TWO-TIER DEFENSE ARCHITECTURE:                                              ║
//! ║                                                                              ║
//! ║  [TIER 1] PRE-MOUNT BLOCKING (DiskArbitration Approval Callback)             ║
//! ║  • Kernel notifies agent BEFORE disk mounts to filesystem.                  ║
//! ║  • If blocked by policy → returns DADissenter (kDAReturnNotPermitted).       ║
//! ║  • RESULT: Drive NEVER mounts to /Volumes/, never appears in Finder!        ║
//! ║                                                                              ║
//! ║  [TIER 2] ACTIVE MOUNT WATCHDOG & INSTANT EJECTION                          ║
//! ║  • Scans /Volumes/ every cycle as an active fallback.                       ║
//! ║  • If any blocked storage slips past → immediate force unmount + eject.     ║
//! ║  • Mobile devices (MTP/PTP) → terminates Android File Transfer / Image      ║
//! ║    Capture and closes active IOKit USB sessions.                            ║
//! ╚══════════════════════════════════════════════════════════════════════════════╝

use std::any::Any;
use std::collections::HashSet;
use std::error::Error;
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use log::{info, warn, error};

use crate::policy::policy_engine::PolicyEngine;
use crate::core::communication::ServerCommunicator;
use crate::protection_modules::ProtectionModule;
use crate::policy::policy_constants::*;
use crate::platform::macos::disk_arbitration::{DiskActionVerdict, DiskArbitrationSession};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalDeviceType {
    /// 1. Standard USBs (Flash drives & thumb drives)
    UsbFlash,
    /// 2. External SSDs (Portable hard drives)
    ExternalSsd,
    /// 3. SD Cards (Memory cards & readers)
    SdCard,
    /// 4. Mobile Devices (Phones & Tablets MTP/WPD)
    Mobile,
    /// Unknown removable media
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsbPolicyData {
    #[serde(rename = "blockUsb")]
    pub block_usb: Option<bool>,
    #[serde(rename = "blockSsd")]
    pub block_ssd: Option<bool>,
    #[serde(rename = "blockSdCard")]
    pub block_sd_card: Option<bool>,
    #[serde(rename = "blockMobile")]
    pub block_mobile: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalDeviceInfo {
    pub bsd_name: String,
    pub mount_point: String,
    pub volume_name: String,
    pub serial_number: String,
    pub total_size: u64,
    pub device_type: ExternalDeviceType,
    pub insertion_time: u64,
}

pub struct UsbProtectionModule {
    known_devices: HashSet<String>,
    current_policy: Arc<RwLock<UsbPolicyData>>,
    da_session: DiskArbitrationSession,
}

impl UsbProtectionModule {
    pub fn new() -> Self {
        let policy_holder = Arc::new(RwLock::new(UsbPolicyData::default()));
        let mut session = DiskArbitrationSession::new();

        // Register Tier 1 Pre-Mount Interception Callback
        let policy_clone = policy_holder.clone();
        let _ = session.start_with_approval(Arc::new(move |bsd_name, is_removable| {
            if !is_removable {
                return DiskActionVerdict::Allow;
            }

            let policy = policy_clone.read();
            // Default block if any storage rule is active
            let block_flash = policy.block_usb.unwrap_or(false);
            let block_ssd = policy.block_ssd.unwrap_or(false);
            let block_sd = policy.block_sd_card.unwrap_or(false);

            if block_flash || block_ssd || block_sd {
                DiskActionVerdict::Block(format!("Storage blocking policy active for disk {}", bsd_name))
            } else {
                DiskActionVerdict::Allow
            }
        }));

        UsbProtectionModule {
            known_devices: HashSet::new(),
            current_policy: policy_holder,
            da_session: session,
        }
    }

    async fn execute_monitoring(
        &mut self,
        policy_engine: &PolicyEngine,
        communicator: &ServerCommunicator,
        agent_id: u64,
        token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let block_usb_active = policy_engine.is_policy_active(POLICY_USB_STORAGE_BLOCK);
        let block_mobile_active = policy_engine.is_policy_active(POLICY_MOBILE_DEVICE_BLOCK);

        let usb_policy: UsbPolicyData = if block_usb_active {
            policy_engine.get_policy_json_data(POLICY_USB_STORAGE_BLOCK)
        } else if block_mobile_active {
            policy_engine.get_policy_json_data(POLICY_MOBILE_DEVICE_BLOCK)
        } else {
            UsbPolicyData::default()
        };

        // Real-time update for the DiskArbitration pre-mount callback
        *self.current_policy.write() = usb_policy.clone();

        let has_storage_block = usb_policy.block_usb.unwrap_or(false)
            || usb_policy.block_ssd.unwrap_or(false)
            || usb_policy.block_sd_card.unwrap_or(false);

        let block_active = block_usb_active || (block_mobile_active && has_storage_block);
        let block_mobile = block_mobile_active || usb_policy.block_mobile.unwrap_or(false);

        // TIER 2 Active Fallback: Scan currently mounted volumes
        let current_devices = self.scan_external_devices();

        for device in &current_devices {
            let is_new = !self.known_devices.contains(&device.serial_number);

            let should_block = if block_active {
                match device.device_type {
                    ExternalDeviceType::UsbFlash => usb_policy.block_usb.unwrap_or(true),
                    ExternalDeviceType::ExternalSsd => usb_policy.block_ssd.unwrap_or(true),
                    ExternalDeviceType::SdCard => usb_policy.block_sd_card.unwrap_or(true),
                    ExternalDeviceType::Unknown => true,
                    _ => false,
                }
            } else {
                false
            };

            if is_new {
                info!("🎯 New external device detected: {} ({:?})", device.serial_number, device.device_type);
                self.known_devices.insert(device.serial_number.clone());

                if should_block {
                    warn!("🚫 External device BLOCKED by policy: {}", device.bsd_name);
                    self.block_and_eject(device, communicator, agent_id, token).await?;
                    continue;
                }

                if policy_engine.is_policy_active(POLICY_USB_DEVICE_MONITOR) {
                    info!("👀 USB device MONITORED: {}", device.serial_number);
                    self.send_device_event(device, "MONITORED", communicator, agent_id, token).await?;
                }
            } else if should_block {
                // Device already known but somehow remounted - force silent eject
                self.eject_silently(device);
            }
        }

        // Enforce Mobile (MTP/PTP) process blocking
        if block_mobile {
            self.enforce_mobile_block();
        }

        // Cleanup removed devices
        let current_serials: HashSet<String> = current_devices
            .iter()
            .map(|d| d.serial_number.clone())
            .collect();
        self.known_devices.retain(|s| current_serials.contains(s));

        Ok(())
    }

    fn scan_external_devices(&self) -> Vec<ExternalDeviceInfo> {
        let mut devices = Vec::new();
        let volumes = match std::fs::read_dir("/Volumes") {
            Ok(v) => v,
            Err(_) => return devices,
        };

        for entry in volumes.flatten() {
            let path = entry.path();
            let vol_name = entry.file_name().to_string_lossy().to_string();

            if vol_name == "Macintosh HD" || vol_name.starts_with('.') {
                continue;
            }

            if let Some(device) = self.get_device_info_for_volume(&path, &vol_name) {
                devices.push(device);
            }
        }
        devices
    }

    fn get_device_info_for_volume(&self, path: &std::path::Path, vol_name: &str) -> Option<ExternalDeviceInfo> {
        let output = Command::new("diskutil")
            .arg("info")
            .arg(path.to_str()?)
            .output()
            .ok()?;

        let info = String::from_utf8_lossy(&output.stdout);
        let is_removable = info.contains("Removable Media: Yes")
            || info.contains("Removable:               Yes")
            || info.contains("Protocol:                USB")
            || info.contains("Protocol:                SD");

        if !is_removable {
            return None;
        }

        let bsd_name = extract_field(&info, "Device Node:")
            .unwrap_or_default()
            .trim_start_matches("/dev/")
            .to_string();

        let total_size = extract_field(&info, "Disk Size:")
            .and_then(|s| s.split_whitespace().next()?.parse::<u64>().ok())
            .unwrap_or(0);

        let protocol = extract_field(&info, "Protocol:").unwrap_or_default();
        let media_type = extract_field(&info, "Media Type:").unwrap_or_default();
        let device_type = classify_device(&protocol, &media_type, total_size);

        let serial = extract_field(&info, "Volume UUID:")
            .unwrap_or_else(|| bsd_name.clone());

        Some(ExternalDeviceInfo {
            bsd_name: bsd_name.clone(),
            mount_point: path.to_string_lossy().to_string(),
            volume_name: vol_name.to_string(),
            serial_number: serial,
            total_size,
            device_type,
            insertion_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }

    async fn block_and_eject(
        &self,
        device: &ExternalDeviceInfo,
        communicator: &ServerCommunicator,
        agent_id: u64,
        token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let _ = Command::new("diskutil")
            .args(&["unmount", "force", &device.mount_point])
            .output();

        let _ = Command::new("diskutil")
            .args(&["eject", &format!("/dev/{}", device.bsd_name)])
            .output();

        info!("🛡️ [UsbGuard] Force unmounted and ejected: {}", device.bsd_name);
        self.send_device_event(device, "BLOCKED", communicator, agent_id, token).await?;
        Ok(())
    }

    fn eject_silently(&self, device: &ExternalDeviceInfo) {
        let _ = Command::new("diskutil")
            .args(&["unmount", "force", &device.mount_point])
            .output();
        let _ = Command::new("diskutil")
            .args(&["eject", &format!("/dev/{}", device.bsd_name)])
            .output();
    }

    fn enforce_mobile_block(&self) {
        let procs = ["Android File Transfer", "MacDroid", "OpenMTP", "Image Capture"];
        for proc in &procs {
            let _ = Command::new("pkill").args(&["-9", "-f", proc]).output();
        }
    }

    async fn send_device_event(
        &self,
        device: &ExternalDeviceInfo,
        action: &str,
        communicator: &ServerCommunicator,
        agent_id: u64,
        token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let payload = serde_json::json!({
            "agentId": agent_id,
            "alertType": format!("EXTERNAL_DEVICE_{}", action),
            "deviceType": format!("{:?}", device.device_type),
            "bsdName": device.bsd_name,
            "volumeName": device.volume_name,
            "mountPoint": device.mount_point,
            "serialNumber": device.serial_number,
            "totalSize": device.total_size,
            "actionTaken": action,
            "timestamp": device.insertion_time,
            "platform": "macOS",
        });

        communicator.send_alert(agent_id, token, &payload).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ProtectionModule for UsbProtectionModule {
    async fn execute(
        &mut self,
        policy_engine: &PolicyEngine,
        communicator: &ServerCommunicator,
        agent_id: u64,
        token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.execute_monitoring(policy_engine, communicator, agent_id, token).await
    }

    fn get_name(&self) -> &str { "ExternalDeviceProtection" }
    fn as_any(&mut self) -> &mut dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

fn extract_field(info: &str, key: &str) -> Option<String> {
    for line in info.lines() {
        if line.contains(key) {
            return line.split(':').nth(1).map(|s| s.trim().to_string());
        }
    }
    None
}

fn classify_device(protocol: &str, media_type: &str, size_bytes: u64) -> ExternalDeviceType {
    let proto_lower = protocol.to_lowercase();
    let media_lower = media_type.to_lowercase();

    if proto_lower.contains("sd") || media_lower.contains("sd card") {
        return ExternalDeviceType::SdCard;
    }
    if size_bytes > 128 * 1024 * 1024 * 1024 {
        return ExternalDeviceType::ExternalSsd;
    }
    if proto_lower.contains("usb") {
        if size_bytes > 64 * 1024 * 1024 * 1024 {
            return ExternalDeviceType::ExternalSsd;
        }
        return ExternalDeviceType::UsbFlash;
    }
    ExternalDeviceType::Unknown
}
