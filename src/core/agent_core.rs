// core/agent_core.rs
//
// Main agent coordinator for macOS.
// Loads and schedules all 5 active protection modules:
// 1. External Devices (Standard USBs, External SSDs, SD Cards, Mobile Devices)
// 2. Web Protection (Activity Monitoring, URL Blocking, Partial Access)
// 3. File Protection (Filesystem protection with Endpoint Security)
// 4. App Usage Monitoring (NSWorkspace active application tracking)
// 5. Security Monitor / OCR (Screen capture + sensitive data rule engine)
//
// Notice: UEBA is explicitly omitted per system design.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::core::capabilities::PolicyCapability;
use crate::core::communication::ServerCommunicator;
use crate::core::file_logger::FileLogger;
use crate::policy::policy_engine::PolicyEngine;
use crate::policy::policy_store::PolicyStore;
use crate::protection_modules::ProtectionModule;
use crate::protection_modules::usb_protection::usb_guard::UsbProtectionModule;
use crate::protection_modules::web_filter::mitm_proxy::WebProtectionModule;
use crate::protection_modules::file_protection::file_protection_module::FileProtectionModule;
use crate::protection_modules::app_usage::app_usage_module::AppUsageModule;
use crate::protection_modules::security_monitor::monitor::SecurityMonitorModule;

pub struct AgentCore {
    pub agent_id: u64,
    pub token: String,
    pub communicator: Arc<RwLock<ServerCommunicator>>,
    pub policy_engine: Arc<RwLock<PolicyEngine>>,
    pub protection_modules: Arc<RwLock<HashMap<String, Box<dyn ProtectionModule + Send + Sync>>>>,
}

impl AgentCore {
    pub fn new(agent_id: u64, token: String, communicator: ServerCommunicator) -> Self {
        AgentCore {
            agent_id,
            token,
            communicator: Arc::new(RwLock::new(communicator)),
            policy_engine: Arc::new(RwLock::new(PolicyEngine::new())),
            protection_modules: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initializes all 5 macOS DLP protection modules
    pub async fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        FileLogger::info("Initializing macOS AgentCore and protection modules...");

        // Load cached policies from disk if available
        if let Ok(cached) = PolicyStore::load() {
            if !cached.is_empty() {
                self.policy_engine.write().await.update_policies(cached);
                FileLogger::info("Loaded offline cached policies successfully");
            }
        }

        // Register agent capabilities with server
        let comm = self.communicator.read().await;
        let caps = PolicyCapability::all_capabilities();
        let _ = comm.register_capabilities(self.agent_id, &self.token, &caps).await;
        drop(comm);

        // Register protection modules
        let mut modules = self.protection_modules.write().await;

        // 1. External Device Protection (Standard USB, External SSD, SD Card, Mobile MTP)
        modules.insert("ExternalDeviceProtection".to_string(), Box::new(UsbProtectionModule::new()));

        // 2. Web Protection (Activity Monitoring, URL Blocking, Partial Access)
        modules.insert("WebProtection".to_string(), Box::new(WebProtectionModule::new()));

        // 3. File Protection (Filesystem protection with Endpoint Security)
        modules.insert("FileProtection".to_string(), Box::new(FileProtectionModule::new()));

        // 4. App Usage Monitoring (NSWorkspace active app tracking)
        modules.insert("AppUsageMonitor".to_string(), Box::new(AppUsageModule::new()));

        // 5. Security Monitor / OCR (Screen capture + sensitive data rule engine)
        modules.insert("SecurityMonitor".to_string(), Box::new(SecurityMonitorModule::new(self.agent_id)));

        FileLogger::info("All 5 macOS DLP protection modules registered successfully.");
        Ok(())
    }

    /// Runs continuous protection and policy polling loops
    pub async fn run_protection_loop(&self, running: Arc<std::sync::atomic::AtomicBool>) {
        FileLogger::info("Starting macOS AgentCore protection loop...");

        let mut policy_ticker = tokio::time::interval(Duration::from_secs(30));
        let mut module_ticker = tokio::time::interval(Duration::from_secs(2));

        while running.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::select! {
                // Synchronize policies from server every 30 seconds
                _ = policy_ticker.tick() => {
                    let comm = self.communicator.read().await;
                    match comm.fetch_policies(self.agent_id, &self.token).await {
                        Ok(policies) => {
                            let _ = PolicyStore::save(&policies);
                            self.policy_engine.write().await.update_policies(policies);
                            FileLogger::debug("Synchronized latest policies from server");
                        }
                        Err(e) => {
                            if e.to_string().contains("AGENT_DELETED") {
                                FileLogger::warn("Agent deleted on server. Triggering self-destruct...");
                                crate::core::self_destruct::trigger_self_destruct("Agent deleted on backend");
                            }
                            FileLogger::warn(&format!("Policy fetch failed: {}", e));
                        }
                    }
                }

                // Execute all protection modules every 2 seconds
                _ = module_ticker.tick() => {
                    let engine = self.policy_engine.read().await.clone();
                    let comm = self.communicator.read().await.clone();
                    let mut modules = self.protection_modules.write().await;

                    for (name, module) in modules.iter_mut() {
                        if let Err(e) = module.execute(&engine, &comm, self.agent_id, &self.token).await {
                            FileLogger::warn(&format!("Module {} execution error: {}", name, e));
                        }
                    }
                }
            }
        }

        FileLogger::info("AgentCore protection loop exited cleanly.");
    }
}
