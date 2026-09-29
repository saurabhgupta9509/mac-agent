// policy/policy_store.rs
//
// Local persistent cache for policies on macOS.
// Allows the agent to enforce DLP policies even when offline or before server connection.

use std::path::PathBuf;
use crate::policy::policy_engine::Policy;
use crate::core::file_logger::FileLogger;

pub struct PolicyStore;

impl PolicyStore {
    fn path() -> PathBuf {
        PathBuf::from("/Library/Application Support/DLPAgent/policies.json")
    }

    pub fn save(policies: &[Policy]) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(policies)?;
        std::fs::write(&path, json)?;
        FileLogger::debug(&format!("PolicyStore: saved {} policies to disk", policies.len()));
        Ok(())
    }

    pub fn load() -> Result<Vec<Policy>, Box<dyn std::error::Error>> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let data = std::fs::read_to_string(&path)?;
        let policies: Vec<Policy> = serde_json::from_str(&data)?;
        FileLogger::info(&format!("PolicyStore: loaded {} cached policies from disk", policies.len()));
        Ok(policies)
    }
}
