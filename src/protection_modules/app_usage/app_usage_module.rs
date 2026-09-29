//! protection_modules/app_usage/app_usage_module.rs
//!
//! ╔══════════════════════════════════════════════════════════════════════════════╗
//! ║         macOS App Usage Monitoring (Session-Based Fast Sync)                 ║
//! ║                                                                              ║
//! ║  FAST SYNC & EVENT-BASED UPDATES:                                           ║
//! ║  • On App Switch (Chrome -> Slack): Immediately ends Chrome session,        ║
//! ║    records duration, and flushes to pending batch.                          ║
//! ║  • Sync interval changed from 5 minutes to 30 SECONDS!                      ║
//! ║  • Real-time updates delivered to DLP Dashboard promptly.                   ║
//! ╚══════════════════════════════════════════════════════════════════════════════╝

use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use log::info;
use serde::{Deserialize, Serialize};

use crate::core::communication::ServerCommunicator;
use crate::policy::policy_engine::PolicyEngine;
use crate::policy::policy_constants::*;
use crate::protection_modules::ProtectionModule;

// Fast 30-second reporting interval (instead of 300s / 5 minutes)
const FAST_REPORT_INTERVAL_SECS: f64 = 30.0;
const IDLE_THRESHOLD_SECS: f64       = 60.0;
const MIN_APP_TIME_SECS: f64         = 1.0;
const IPC_FILE: &str                 = "/Library/Application Support/DLPAgent/current_app.txt";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUsageData {
    pub timestamp: String,
    pub agent_id: u64,
    pub current_app: String,
    pub current_session_duration: f64,
    pub total_apps_tracked: u32,
    pub active_usage_time: f64,
    pub top_apps: Vec<serde_json::Value>,
    pub category_breakdown: HashMap<String, f64>,
    pub platform: String,
}

#[derive(Debug)]
struct AppSession {
    start_time: f64,
    total_time: f64,
    session_count: u32,
}

pub struct AppUsageModule {
    current_app: Option<String>,
    app_start_time: Option<f64>,
    app_sessions: HashMap<String, AppSession>,
    last_report_time: f64,
    has_unsent_data: bool,
    idle: bool,
}

impl AppUsageModule {
    pub fn new() -> Self {
        AppUsageModule {
            current_app: None,
            app_start_time: None,
            app_sessions: HashMap::new(),
            last_report_time: current_time_secs(),
            has_unsent_data: false,
            idle: false,
        }
    }

    async fn execute_monitoring(
        &mut self,
        policy_engine: &PolicyEngine,
        communicator: &ServerCommunicator,
        agent_id: u64,
        token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if !policy_engine.is_policy_active(POLICY_APP_USAGE_MONITOR) {
            return Ok(());
        }

        let now = current_time_secs();
        let idle_secs = get_idle_time_secs();
        self.idle = idle_secs > IDLE_THRESHOLD_SECS;

        let active_app = if self.idle {
            "Idle".to_string()
        } else {
            get_frontmost_app()
        };

        // Write IPC state for user UI
        let _ = std::fs::create_dir_all("/Library/Application Support/DLPAgent");
        let _ = std::fs::write(IPC_FILE, &active_app);

        // EVENT-DRIVEN SESSION CLOSE: User switched application
        if Some(&active_app) != self.current_app.as_ref() {
            if let (Some(prev_app), Some(start)) = (self.current_app.take(), self.app_start_time.take()) {
                let duration = now - start;
                if duration >= MIN_APP_TIME_SECS {
                    let entry = self.app_sessions.entry(prev_app.clone()).or_insert(AppSession {
                        start_time: start,
                        total_time: 0.0,
                        session_count: 0,
                    });
                    entry.total_time += duration;
                    entry.session_count += 1;
                    self.has_unsent_data = true;
                    info!("[AppUsage] App switched from '{}' (session: {:.1}s) to '{}'", prev_app, duration, active_app);
                }
            }
            self.current_app = Some(active_app.clone());
            self.app_start_time = Some(now);
        }

        // FAST REPORTING: Sync every 30 seconds if we have active data
        if (now - self.last_report_time >= FAST_REPORT_INTERVAL_SECS && self.has_unsent_data)
            || (now - self.last_report_time >= 60.0)
        {
            self.send_usage_report(communicator, agent_id, token).await?;
            self.last_report_time = now;
            self.has_unsent_data = false;
        }

        Ok(())
    }

    async fn send_usage_report(
        &mut self,
        communicator: &ServerCommunicator,
        agent_id: u64,
        token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Accumulate active session duration for current running app
        let now = current_time_secs();
        if let (Some(ref curr), Some(start)) = (&self.current_app, self.app_start_time) {
            let active_dur = now - start;
            if active_dur > 0.0 {
                let entry = self.app_sessions.entry(curr.clone()).or_insert(AppSession {
                    start_time: start,
                    total_time: 0.0,
                    session_count: 0,
                });
                entry.total_time += active_dur;
                self.app_start_time = Some(now); // reset slice start
            }
        }

        let mut top_apps: Vec<(&String, &AppSession)> = self.app_sessions.iter().collect();
        top_apps.sort_by(|a, b| b.1.total_time.partial_cmp(&a.1.total_time).unwrap());

        let top_apps_json: Vec<serde_json::Value> = top_apps.iter().take(10).map(|(name, session)| {
            serde_json::json!({
                "appName": name,
                "totalSeconds": session.total_time as u64,
                "sessionCount": session.session_count.max(1),
                "category": categorize_app(name),
            })
        }).collect();

        let mut categories: HashMap<String, f64> = HashMap::new();
        for (name, session) in &self.app_sessions {
            let cat = categorize_app(name);
            *categories.entry(cat).or_insert(0.0) += session.total_time;
        }

        let total_active: f64 = self.app_sessions.values().map(|s| s.total_time).sum();

        let current = self.current_app.clone().unwrap_or_default();
        let payload = AppUsageData {
            timestamp: chrono::Local::now().to_rfc3339(),
            agent_id,
            current_app: current,
            current_session_duration: 0.0,
            total_apps_tracked: self.app_sessions.len() as u32,
            active_usage_time: total_active,
            top_apps: top_apps_json,
            category_breakdown: categories,
            platform: "macOS".to_string(),
        };

        communicator.send_app_usage(agent_id, token, &payload).await?;
        info!("[AppUsage] Fast sync report sent: {} apps, {:.0}s active time", payload.total_apps_tracked, payload.active_usage_time);

        self.app_sessions.clear();
        Ok(())
    }
}

fn get_frontmost_app() -> String {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to get name of first application process whose frontmost is true")
        .output();

    if let Ok(o) = output {
        let name = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !name.is_empty() && name != "null" {
            return name;
        }
    }
    "Unknown".to_string()
}

fn get_idle_time_secs() -> f64 {
    let output = std::process::Command::new("ioreg")
        .args(&["-c", "IOHIDSystem"])
        .output();

    if let Ok(o) = output {
        let text = String::from_utf8_lossy(&o.stdout);
        for line in text.lines() {
            if line.contains("HIDIdleTime") {
                if let Some(val) = line.split('=').nth(1) {
                    if let Ok(ns) = val.trim().parse::<u64>() {
                        return ns as f64 / 1_000_000_000.0;
                    }
                }
            }
        }
    }
    0.0
}

fn categorize_app(app_name: &str) -> String {
    let lower = app_name.to_lowercase();
    match lower.as_str() {
        n if n.contains("chrome") || n.contains("safari") || n.contains("firefox") || n.contains("brave") => "Browser",
        n if n.contains("slack") || n.contains("teams") || n.contains("zoom") || n.contains("discord") => "Communication",
        n if n.contains("code") || n.contains("xcode") || n.contains("terminal") || n.contains("iterm") => "Development",
        n if n.contains("word") || n.contains("excel") || n.contains("pages") || n.contains("numbers") => "Productivity",
        _ => "Other",
    }.to_string()
}

fn current_time_secs() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64()
}

#[async_trait::async_trait]
impl ProtectionModule for AppUsageModule {
    async fn execute(
        &mut self,
        policy_engine: &PolicyEngine,
        communicator: &ServerCommunicator,
        agent_id: u64,
        token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.execute_monitoring(policy_engine, communicator, agent_id, token).await
    }

    fn get_name(&self) -> &str { "AppUsageMonitor" }
    fn as_any(&mut self) -> &mut dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
