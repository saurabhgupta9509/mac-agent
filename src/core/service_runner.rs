// core/service_runner.rs
//
// Manages LaunchDaemon lifecycle and background agent execution.
// Loads credentials, maintains heartbeats, and runs AgentCore until stopped.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use crate::core::agent_core::AgentCore;
use crate::core::communication::ServerCommunicator;
use crate::core::credential_store::CredentialStore;
use crate::core::file_logger::FileLogger;

const ADMIN_STOP_FILE: &str = "/Library/Application Support/DLPAgent/admin_stopped";

pub fn is_service_stopped_by_admin() -> bool {
    std::path::Path::new(ADMIN_STOP_FILE).exists()
}

pub fn mark_service_started_by_admin() {
    let _ = std::fs::remove_file(ADMIN_STOP_FILE);
}

pub async fn run_agent_until_stopped(running: Arc<AtomicBool>) {
    FileLogger::info("service_runner: Loading credentials for macOS daemon...");

    let creds = loop {
        match CredentialStore::load() {
            Ok(c) => {
                FileLogger::info(&format!("service_runner: Credentials loaded successfully for agent ID {}", c.agent_id));
                break c;
            }
            Err(_) => {
                if !running.load(Ordering::SeqCst) {
                    return;
                }
                // Check every 3 seconds for credentials to be written by Setup Wizard
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    };

    let comm = ServerCommunicator::new(creds.server_url.clone());
    let token = creds.token.unwrap_or_default();

    let mut agent = AgentCore::new(creds.agent_id, token, comm.clone());

    if let Err(e) = agent.initialize().await {
        FileLogger::error(&format!("Failed to initialize AgentCore: {}", e));
        return;
    }

    // Spawn heartbeat worker
    let hb_running = running.clone();
    let hb_comm = comm.clone();
    let hb_agent_id = creds.agent_id;
    let hb_token = agent.token.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        while hb_running.load(Ordering::SeqCst) {
            interval.tick().await;
            match hb_comm.send_heartbeat(hb_agent_id, &hb_token).await {
                Ok(true) => FileLogger::debug("Daemon heartbeat OK"),
                Ok(false) => FileLogger::warn("Daemon heartbeat rejected by server"),
                Err(e) => {
                    if e.to_string().contains("AGENT_DELETED") {
                        FileLogger::warn("Heartbeat revealed agent deleted on server. Self-destructing...");
                        crate::core::self_destruct::trigger_self_destruct("Agent deleted from backend dashboard");
                    }
                    FileLogger::warn(&format!("Heartbeat error: {}", e));
                }
            }
        }
    });

    // Spawn WebSocket tunnel worker for live dashboard controls (Explorer, Drives, etc.)
    let ws_running = running.clone();
    let ws_server_url = creds.server_url.clone();
    let ws_agent_id = creds.agent_id;
    let ws_token = agent.token.clone();

    tokio::spawn(async move {
        crate::core::agent_ws::AgentWebSocket::run_loop(
            ws_server_url,
            ws_agent_id,
            ws_token,
            ws_running,
        ).await;
    });

    // Run core protection loop
    agent.run_protection_loop(running).await;
}
