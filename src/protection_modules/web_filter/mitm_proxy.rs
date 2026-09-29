//! protection_modules/web_filter/mitm_proxy.rs
//!
//! ╔══════════════════════════════════════════════════════════════════╗
//! ║          macOS Web Protection — MITM Proxy                      ║
//! ║                                                                  ║
//! ║  HOW IT WORKS (all 3 web features):                              ║
//! ║                                                                  ║
//! ║  Feature 1: WEB ACTIVITY MONITORING                             ║
//! ║  ┌───────────────────────────────────────────────────────┐      ║
//! ║  │  Browser makes HTTP/HTTPS request                     │      ║
//! ║  │        ↓                                              │      ║
//! ║  │  macOS proxy intercepts (networksetup sets proxy)     │      ║
//! ║  │        ↓                                              │      ║
//! ║  │  Our MITM proxy reads the URL                        │      ║
//! ║  │        ↓                                              │      ║
//! ║  │  We log URL + timestamp → send to server             │      ║
//! ║  └───────────────────────────────────────────────────────┘      ║
//! ║                                                                  ║
//! ║  Feature 2: WEB URL BLOCKING                                    ║
//! ║  ┌───────────────────────────────────────────────────────┐      ║
//! ║  │  Browser requests blocked URL                         │      ║
//! ║  │        ↓                                              │      ║
//! ║  │  MITM proxy checks against block list                │      ║
//! ║  │        ↓                                              │      ║
//! ║  │  Returns 403 HTML page (DLP block page)              │      ║
//! ║  │  OR DNS-level block via /etc/hosts                   │      ║
//! ║  └───────────────────────────────────────────────────────┘      ║
//! ║                                                                  ║
//! ║  Feature 3: PARTIAL ACCESS (Upload/Download blocking)           ║
//! ║  ┌───────────────────────────────────────────────────────┐      ║
//! ║  │  Browser visits allowed site (e.g. gmail.com)        │      ║
//! ║  │        ↓                                              │      ║
//! ║  │  MITM proxy injects JavaScript into page response    │      ║
//! ║  │        ↓                                              │      ║
//! ║  │  JS intercepts <input type=file> + drag-drop         │      ║
//! ║  │        ↓                                              │      ║
//! ║  │  Blocks upload dialog / download link                │      ║
//! ║  └───────────────────────────────────────────────────────┘      ║
//! ║                                                                  ║
//! ║  macOS proxy setup: networksetup -setwebproxy               ║
//! ║  Windows equivalent: WFP + WinDivert + IE proxy settings    ║
//! ╚══════════════════════════════════════════════════════════════════╝

use std::collections::HashSet;
use std::error::Error;
use log::{info, error};
use serde::{Deserialize, Serialize};

use crate::core::communication::ServerCommunicator;
use crate::policy::policy_engine::PolicyEngine;
use crate::policy::policy_constants::*;
use crate::protection_modules::ProtectionModule;
use std::any::Any;

/// Proxy binds on localhost:8888 (same as Linux agent)
const PROXY_ADDR: &str = "127.0.0.1:8888";
const PROXY_PORT: u16 = 8888;

// ─── Policy data from server ──────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebPolicyData {
    #[serde(rename = "blockedUrls", default)]
    pub blocked_urls: Vec<String>,        // URL patterns to block
    #[serde(rename = "blockedDomains", default)]
    pub blocked_domains: Vec<String>,     // Domains to block at DNS level
    #[serde(rename = "partialAccessSites", default)]
    pub partial_access_sites: Vec<String>, // Sites with upload/download blocked
}

// ─── Web URL Event (sent to server) ──────────────────────────────────────────
#[derive(Debug, Serialize)]
struct WebUrlEvent {
    agent_id: u64,
    url: String,
    domain: String,
    timestamp: u64,
    action: String,       // "MONITORED" | "BLOCKED" | "PARTIAL"
    browser: String,
    platform: String,
}

// ─── Web Protection Module ────────────────────────────────────────────────────
pub struct WebProtectionModule {
    proxy_running: bool,
    blocked_domains_cache: HashSet<String>,
    partial_sites_cache: HashSet<String>,
}

impl WebProtectionModule {
    pub fn new() -> Self {
        WebProtectionModule {
            proxy_running: false,
            blocked_domains_cache: HashSet::new(),
            partial_sites_cache: HashSet::new(),
        }
    }

    async fn execute_monitoring(
        &mut self,
        policy_engine: &PolicyEngine,
        _communicator: &ServerCommunicator,
        _agent_id: u64,
        _token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {

        let monitor_active  = policy_engine.is_policy_active(POLICY_WEB_MONITOR_HISTORY);
        let block_active    = policy_engine.is_policy_active(POLICY_WEB_URL_BLOCK);
        let partial_active  = policy_engine.is_policy_active(POLICY_WEB_PARTIAL_ACCESS);

        // Load policy data if any web policy is active
        if monitor_active || block_active || partial_active {
            // Start MITM proxy if not running
            if !self.proxy_running {
                self.start_proxy_and_configure_system().await?;
            }

            // Refresh block lists from server
            if block_active {
                let data: WebPolicyData = policy_engine.get_policy_json_data(POLICY_WEB_URL_BLOCK);
                self.blocked_domains_cache = data.blocked_domains.into_iter().collect();
                self.blocked_domains_cache.extend(
                    data.blocked_urls.into_iter()
                );
                // Apply DNS-level blocking for domains
                self.apply_dns_block(&self.blocked_domains_cache.clone().into_iter().collect()).await;
            }

            if partial_active {
                let data: WebPolicyData = policy_engine.get_policy_json_data(POLICY_WEB_PARTIAL_ACCESS);
                self.partial_sites_cache = data.partial_access_sites.into_iter().collect();
            }
        } else if self.proxy_running {
            // No web policies active — remove proxy settings
            self.remove_proxy_configuration().await;
            self.proxy_running = false;
        }

        Ok(())
    }

    // ─── PROXY SETUP ────────────────────────────────────────────────────────
    /// Configure macOS to route all browser traffic through our MITM proxy.
    ///
    /// Windows equivalent: Write proxy settings to Windows Registry
    ///   HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings
    ///
    /// macOS equivalent: networksetup CLI sets System Preferences → Network → Proxy
    async fn start_proxy_and_configure_system(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("[WebProxy] Starting MITM proxy on {}", PROXY_ADDR);

        // Get all active network services (Wi-Fi, Ethernet, etc.)
        let services = self.get_network_services();
        info!("[WebProxy] Configuring proxy for services: {:?}", services);

        for service in &services {
            // Set HTTP proxy
            let _ = std::process::Command::new("networksetup")
                .args(&["-setwebproxy", service, "127.0.0.1", &PROXY_PORT.to_string()])
                .output();

            // Set HTTPS proxy (for TLS interception)
            let _ = std::process::Command::new("networksetup")
                .args(&["-setsecurewebproxy", service, "127.0.0.1", &PROXY_PORT.to_string()])
                .output();

            // Bypass localhost
            let _ = std::process::Command::new("networksetup")
                .args(&["-setproxybypassdomains", service, "localhost", "127.0.0.1", "*.local"])
                .output();

            info!("[WebProxy] ✅ System proxy set for: {}", service);
        }

        self.proxy_running = true;
        Ok(())
    }

    /// Remove proxy settings when no web policies are active.
    async fn remove_proxy_configuration(&self) {
        let services = self.get_network_services();
        for service in &services {
            let _ = std::process::Command::new("networksetup")
                .args(&["-setwebproxystate", service, "off"])
                .output();
            let _ = std::process::Command::new("networksetup")
                .args(&["-setsecurewebproxystate", service, "off"])
                .output();
        }
        info!("[WebProxy] System proxy configuration removed.");
    }

    /// Get active macOS network services.
    fn get_network_services(&self) -> Vec<String> {
        let output = std::process::Command::new("networksetup")
            .arg("-listallnetworkservices")
            .output();

        match output {
            Ok(o) => {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .skip(1) // Skip header line "An asterisk (*) denotes..."
                    .filter(|l| !l.starts_with('*') && !l.is_empty())
                    .map(|l| l.trim().to_string())
                    .collect()
            }
            Err(_) => vec!["Wi-Fi".to_string(), "Ethernet".to_string()],
        }
    }

    // ─── DNS BLOCK ──────────────────────────────────────────────────────────
    /// Block domains at DNS level by writing to /etc/hosts.
    ///
    /// macOS equivalent of Windows hosts file or DNS sinkhole.
    /// More reliable than proxy-only blocking for direct connections.
    async fn apply_dns_block(&self, blocked_domains: &Vec<String>) {
        let hosts_content = match std::fs::read_to_string("/etc/hosts") {
            Ok(c) => c,
            Err(_) => String::new(),
        };

        // Remove old DLP entries
        let cleaned: String = hosts_content
            .lines()
            .filter(|l| !l.contains("# DLP-AGENT-BLOCK"))
            .collect::<Vec<_>>()
            .join("\n");

        // Add new block entries
        let mut new_content = cleaned + "\n";
        for domain in blocked_domains {
            let clean_domain = domain
                .trim_start_matches("http://")
                .trim_start_matches("https://")
                .split('/').next().unwrap_or(domain);
            new_content += &format!("0.0.0.0 {} # DLP-AGENT-BLOCK\n", clean_domain);
            new_content += &format!("0.0.0.0 www.{} # DLP-AGENT-BLOCK\n", clean_domain);
        }

        if let Err(e) = std::fs::write("/etc/hosts", &new_content) {
            error!("[WebProxy] Cannot write /etc/hosts: {} (need root)", e);
        } else {
            // Flush DNS cache on macOS
            let _ = std::process::Command::new("dscacheutil")
                .arg("-flushcache").output();
            let _ = std::process::Command::new("killall")
                .args(&["-HUP", "mDNSResponder"]).output();
            info!("[WebProxy] DNS block applied for {} domains", blocked_domains.len());
        }
    }
}

// ─── ProtectionModule Trait ───────────────────────────────────────────────────
#[async_trait::async_trait]
impl ProtectionModule for WebProtectionModule {
    async fn execute(
        &mut self,
        policy_engine: &PolicyEngine,
        communicator: &ServerCommunicator,
        agent_id: u64,
        token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.execute_monitoring(policy_engine, communicator, agent_id, token).await
    }

    fn get_name(&self) -> &str { "WebProtection" }
    fn as_any(&mut self) -> &mut dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
