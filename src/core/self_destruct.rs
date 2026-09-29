// core/self_destruct.rs
//
// Complete self-destruction routine for macOS DLP Agent.
// Reverts network proxy, removes root certificates, deletes launch daemons,
// purges files, and terminates execution.

use std::process::Command;
use crate::core::file_logger::FileLogger;

pub fn trigger_self_destruct(reason: &str) {
    FileLogger::warn(&format!("💥 [SelfDestruct] INITIATING COMPLETE SELF-DESTRUCTION: {}", reason));

    // 1. Remove Proxy Settings on all active network interfaces
    FileLogger::info("[SelfDestruct] Disabling system proxy configurations...");
    let output = Command::new("networksetup").arg("-listallnetworkservices").output();
    if let Ok(o) = output {
        let text = String::from_utf8_lossy(&o.stdout);
        for line in text.lines().skip(1) {
            let service = line.trim();
            if !service.starts_with('*') && !service.is_empty() {
                let _ = Command::new("networksetup").args(["-setwebproxystate", service, "off"]).output();
                let _ = Command::new("networksetup").args(["-setsecurewebproxystate", service, "off"]).output();
            }
        }
    }

    // 2. Remove Root CA Certificates from macOS System Keychain
    FileLogger::info("[SelfDestruct] Removing CA certificates from System Keychain...");
    let _ = Command::new("security")
        .args(["delete-certificate", "-c", "DLP Agent Root CA", "/Library/Keychains/System.keychain"])
        .output();
    let _ = Command::new("security")
        .args(["delete-certificate", "-c", "DLP Agent Partial Access CA", "/Library/Keychains/System.keychain"])
        .output();

    // 3. Terminate UI helper processes
    FileLogger::info("[SelfDestruct] Terminating UI and helper processes...");
    let _ = Command::new("pkill").args(["-f", "dlp-ui"]).output();
    let _ = Command::new("pkill").args(["-f", "dlp-agent --ui-helper"]).output();

    // 4. Remove /etc/hosts entries added by DLP agent
    FileLogger::info("[SelfDestruct] Cleaning /etc/hosts...");
    if let Ok(hosts) = std::fs::read_to_string("/etc/hosts") {
        let cleaned: String = hosts
            .lines()
            .filter(|l| !l.contains("# DLP-AGENT-BLOCK"))
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write("/etc/hosts", cleaned);
        let _ = Command::new("dscacheutil").arg("-flushcache").output();
    }

    // 5. Unload LaunchDaemon and LaunchAgent
    FileLogger::info("[SelfDestruct] Unloading LaunchDaemons...");
    let daemon_plist = "/Library/LaunchDaemons/com.dlpagent.daemon.plist";
    let _ = Command::new("launchctl").args(["bootout", "system", daemon_plist]).output();
    let _ = std::fs::remove_file(daemon_plist);

    let agent_plist = "/Library/LaunchAgents/com.dlpagent.agent.plist";
    let _ = std::fs::remove_file(agent_plist);

    // 6. Delete application directory
    FileLogger::info("[SelfDestruct] Purging application files...");
    let _ = std::fs::remove_dir_all("/Library/Application Support/DLPAgent");
    let _ = std::fs::remove_dir_all("/Library/DLPAgent");

    FileLogger::info("[SelfDestruct] Self-destruction complete. Exiting.");
    std::process::exit(0);
}
