// policy/policy_engine.rs
//
// Core policy evaluation engine for macOS DLP Agent.
// Matches server-side capability definitions and JSON structures.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{Local, Timelike, Datelike, Weekday};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::Deserialize<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub code: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub description: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub category: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub action: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub target: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub severity: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_active: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub policy_data: String,
    #[serde(default)]
    pub schedule_type: Option<String>,
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(default)]
    pub end_time: Option<String>,
    #[serde(default)]
    pub active_days: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub bypass_start: Option<i64>,
    #[serde(default)]
    pub bypass_until: Option<i64>,
}

#[derive(Clone)]
pub struct PolicyEngine {
    policies: Vec<Policy>,
    file_policies: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
            file_policies: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn update_policies(&mut self, policies: Vec<Policy>) {
        self.policies = policies;
        debug!("🔄 Updated PolicyEngine with {} policies", self.policies.len());
    }

    pub fn is_policy_active(&self, policy_code: &str) -> bool {
        self.policies.iter().any(|p| {
            p.code == policy_code && p.is_active && self.is_within_schedule(p)
        })
    }

    pub fn get_policy(&self, policy_code: &str) -> Option<&Policy> {
        self.policies.iter().find(|p| p.code == policy_code)
    }

    pub fn get_policy_json_data<T: for<'de> serde::Deserialize<'de> + Default>(
        &self,
        policy_code: &str,
    ) -> T {
        if let Some(policy) = self.policies.iter().find(|p| {
            p.code == policy_code && p.is_active && self.is_within_schedule(p)
        }) {
            if policy.policy_data.is_empty() {
                return T::default();
            }
            serde_json::from_str(&policy.policy_data).unwrap_or_else(|e| {
                warn!("Failed to parse policy data for {}: {}. Data: '{}'", policy_code, e, policy.policy_data);
                T::default()
            })
        } else {
            T::default()
        }
    }

    pub fn is_within_schedule(&self, policy: &Policy) -> bool {
        // 1. Temporary Bypass Check
        if let Some(bypass_until) = policy.bypass_until {
            let bypass_start = policy.bypass_start.unwrap_or(0);
            let now_ms = Local::now().timestamp_millis();
            if now_ms >= bypass_start && now_ms < bypass_until {
                return false;
            }
        }

        let schedule_type = match &policy.schedule_type {
            Some(t) => t.as_str(),
            None => "ALWAYS",
        };

        if schedule_type == "ALWAYS" || schedule_type.is_empty() {
            return true;
        }

        let now = Local::now();
        let current_date_str = now.format("%Y-%m-%d").to_string();

        if schedule_type == "RANGE" {
            if let Some(start_date_str) = &policy.start_date {
                if !start_date_str.is_empty() && current_date_str < *start_date_str {
                    return false;
                }
            }
            if let Some(end_date_str) = &policy.end_date {
                if !end_date_str.is_empty() && current_date_str > *end_date_str {
                    return false;
                }
            }
        }

        if schedule_type == "WEEKLY" {
            if let Some(active_days) = &policy.active_days {
                let current_day = match now.weekday() {
                    Weekday::Mon => "MON",
                    Weekday::Tue => "TUE",
                    Weekday::Wed => "WED",
                    Weekday::Thu => "THU",
                    Weekday::Fri => "FRI",
                    Weekday::Sat => "SAT",
                    Weekday::Sun => "SUN",
                };
                if !active_days.to_uppercase().contains(current_day) {
                    return false;
                }
            }
        }

        // Time of Day Check
        if let (Some(start_time), Some(end_time)) = (&policy.start_time, &policy.end_time) {
            if !start_time.is_empty() && !end_time.is_empty() {
                let current_time_str = now.format("%H:%M").to_string();
                if current_time_str < *start_time || current_time_str > *end_time {
                    return false;
                }
            }
        }

        true
    }
}
