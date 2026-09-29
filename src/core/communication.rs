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
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    #[serde(rename = "agentId")]
    pub agent_id: u64,
    #[serde(default)]
    pub username: Option<String>,
    pub token: String,
    #[serde(default)]
    pub status: Option<String>,
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
        let hostname = whoami::devicename();
        let payload = serde_json::json!({
            "username": username,
            "password": password,
            "hostname": hostname,
            "macAddress": "00:00:00:00:00:00",
            "ipAddress": "127.0.0.1",
            "os": "macOS",
        });

        let resp = self.client.post(&url)
            .json(&payload)
            .send()
            .await?;

        let status = resp.status();
        let raw_text = resp.text().await?;
        FileLogger::info(&format!("[Communicator] Login response (HTTP {}): {}", status, raw_text));

        if !status.is_success() {
            if let Ok(err_resp) = serde_json::from_str::<ApiResponse<serde_json::Value>>(&raw_text) {
                if let Some(msg) = err_resp.message {
                    return Err(format!("Login failed: {}", msg).into());
                }
            }
            return Err(format!("Login failed: HTTP {}", status).into());
        }

        // 1. Try decoding as ApiResponse<AuthResponse>
        if let Ok(wrapper) = serde_json::from_str::<ApiResponse<AuthResponse>>(&raw_text) {
            if let Some(auth) = wrapper.data {
                FileLogger::info(&format!("[Communicator] Successfully parsed ApiResponse<AuthResponse>: Agent ID {}", auth.agent_id));
                return Ok(auth);
            }
        }

        // 2. Try decoding as direct AuthResponse
        if let Ok(auth) = serde_json::from_str::<AuthResponse>(&raw_text) {
            FileLogger::info(&format!("[Communicator] Successfully parsed direct AuthResponse: Agent ID {}", auth.agent_id));
            return Ok(auth);
        }

        // 3. Fallback: Parse dynamically using serde_json::Value
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw_text) {
            let data_obj = val.get("data").unwrap_or(&val);
            let agent_id = data_obj.get("agentId")
                .or_else(|| data_obj.get("userId"))
                .or_else(|| data_obj.get("id"))
                .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                .ok_or_else(|| format!("missing field 'agentId' in response: {}", raw_text))?;

            let token = data_obj.get("token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let uname = data_obj.get("username")
                .and_then(|v| v.as_str())
                .unwrap_or(username)
                .to_string();

            return Ok(AuthResponse {
                agent_id,
                username: Some(uname),
                token,
                status: None,
            });
        }

        Err(format!("Failed to parse login response: {}", raw_text).into())
    }

    /// Fetch latest assigned policies from server
    pub async fn fetch_policies(&self, agent_id: u64, token: &str) -> Result<Vec<Policy>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/agent/active-policies?agentId={}", self.base_url, agent_id);
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

        let raw_text = resp.text().await?;
        if let Ok(wrapper) = serde_json::from_str::<ApiResponse<PolicyResponse>>(&raw_text) {
            if let Some(data) = wrapper.data {
                return Ok(data.policies);
            }
        }
        if let Ok(policy_resp) = serde_json::from_str::<PolicyResponse>(&raw_text) {
            return Ok(policy_resp.policies);
        }

        Ok(Vec::new())
    }

    /// Register macOS agent capabilities with backend
    pub async fn register_capabilities(
        &self,
        agent_id: u64,
        token: &str,
        caps: &[PolicyCapability],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/agent/capabilities", self.base_url);
        let request_data = serde_json::json!({
            "agentId": agent_id,
            "capabilities": caps
        });
        let resp = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", token.trim_start_matches("Bearer ")))
            .header("Content-Type", "application/json")
            .json(&request_data)
            .send()
            .await?;

        if resp.status().is_success() {
            FileLogger::info(&format!("[Communicator] Successfully registered {} capabilities with backend", caps.len()));
        } else {
            FileLogger::warn(&format!("[Communicator] Failed to register capabilities: HTTP {}", resp.status()));
        }
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
        let url = format!("{}/api/agent/heartbeat?agentId={}", self.base_url, agent_id);
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
