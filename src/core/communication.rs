// core/communication.rs
//
// HTTP communication client between macOS DLP Agent and central backend server.
// Handles authentication, policy synchronization, alerts, app usage reports,
// and system heartbeats.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use crate::core::capabilities::PolicyCapability;
use crate::policy::policy_engine::Policy;
use crate::core::file_logger::FileLogger;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    #[serde(rename = "agentId")]
    pub agent_id: u64,
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyResponse {
    #[serde(rename = "agentId")]
    pub agent_id: u64,
    pub policies: Vec<Policy>,
    #[serde(default)]
    pub timestamp: u64,
}

#[derive(Clone)]
pub struct ServerCommunicator {
    pub base_url: String,
    client: Client,
}

impl ServerCommunicator {
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .danger_accept_invalid_certs(true) // For internal testing/self-signed certs
            .build()
            .unwrap_or_default();

        ServerCommunicator {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    /// Authenticate agent with server
    pub async fn login(&self, username: &str, password: &str) -> Result<AuthResponse, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/agent/login", self.base_url);
        let payload = serde_json::json!({
            "username": username,
            "password": password,
            "os": "macOS",
        });

        let resp = self.client.post(&url)
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(format!("Login failed: HTTP {}", resp.status()).into());
        }

        let auth: AuthResponse = resp.json().await?;
        Ok(auth)
    }

    /// Fetch latest assigned policies from server
    pub async fn fetch_policies(&self, agent_id: u64, token: &str) -> Result<Vec<Policy>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/agent/policies/{}", self.base_url, agent_id);
        let resp = self.client.get(&url)
            .header("Authorization", format!("Bearer {}", token.trim_start_matches("Bearer ")))
            .send()
            .await?;

        if resp.status().as_u16() == 404 || resp.status().as_u16() == 401 {
            return Err("AGENT_DELETED_OR_INVALID".into());
        }

        if !resp.status().is_success() {
            return Err(format!("Fetch policies failed: HTTP {}", resp.status()).into());
        }

        let policy_resp: PolicyResponse = resp.json().await?;
        Ok(policy_resp.policies)
    }

    /// Register macOS agent capabilities with backend
    pub async fn register_capabilities(
        &self,
        agent_id: u64,
        token: &str,
        caps: &[PolicyCapability],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/agent/capabilities/{}", self.base_url, agent_id);
        let _ = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", token.trim_start_matches("Bearer ")))
            .json(&caps)
            .send()
            .await;
        Ok(())
    }

    /// Send a security alert / violation event to server
    pub async fn send_alert(
        &self,
        _agent_id: u64,
        token: &str,
        payload: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/agent/alerts", self.base_url);
        let resp = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", token.trim_start_matches("Bearer ")))
            .json(payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            FileLogger::warn(&format!("Failed to post alert: HTTP {}", resp.status()));
        }
        Ok(())
    }

    /// Post application usage telemetry report
    pub async fn send_app_usage(
        &self,
        _agent_id: u64,
        token: &str,
        payload: &crate::protection_modules::app_usage::app_usage_module::AppUsageData,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/agent/app-usage", self.base_url);
        let resp = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", token.trim_start_matches("Bearer ")))
            .json(payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            FileLogger::warn(&format!("Failed to post app usage: HTTP {}", resp.status()));
        }
        Ok(())
    }

    /// Send periodic heartbeat
    pub async fn send_heartbeat(&self, agent_id: u64, token: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/agent/heartbeat/{}", self.base_url, agent_id);
        let resp = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", token.trim_start_matches("Bearer ")))
            .send()
            .await?;

        if resp.status().as_u16() == 404 || resp.status().as_u16() == 400 {
            return Err("AGENT_DELETED".into());
        }

        Ok(resp.status().is_success())
    }
}
