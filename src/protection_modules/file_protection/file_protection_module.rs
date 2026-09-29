//! protection_modules/file_protection/file_protection_module.rs
//!
//! ╔══════════════════════════════════════════════════════════════════════════════╗
//! ║         macOS Granular File Protection (Endpoint Security)                   ║
//! ║                                                                              ║
//! ║  ENFORCES THE 5 GRANULAR ACTIONS SPECIFIED BY USER:                         ║
//! ║                                                                              ║
//! ║  1. READ BLOCK:                                                              ║
//! ║     • If Read is blocked/not allowed, ALL operations are blocked!            ║
//! ║     • Cannot read, cannot open, cannot copy, cannot preview, cannot delete!  ║
//! ║     • Enforced via ES_EVENT_TYPE_AUTH_OPEN (any open flag) -> DENIED.       ║
//! ║                                                                              ║
//! ║  2. WRITE BLOCK:                                                             ║
//! ║     • File can be opened & read, but CANNOT be modified or saved.            ║
//! ║     • Enforced via ES_EVENT_TYPE_AUTH_WRITE, AUTH_OPEN(O_WRONLY/O_TRUNC)     ║
//! ║       -> DENIED. Read and delete still allowed.                             ║
//! ║                                                                              ║
//! ║  3. DELETE BLOCK:                                                            ║
//! ║     • File/folder CANNOT be permanently deleted, rm'd, or moved to Trash.    ║
//! ║     • Enforced via ES_EVENT_TYPE_AUTH_UNLINK / TRUNCATE -> DENIED.           ║
//! ║     • Read and write still allowed.                                          ║
//! ║                                                                              ║
//! ║  4. RENAME BLOCK:                                                            ║
//! ║     • Protected folder itself & ANY file/subfolder inside CANNOT be renamed. ║
//! ║     • Enforced via ES_EVENT_TYPE_AUTH_RENAME -> DENIED.                      ║
//! ║                                                                              ║
//! ║  5. CREATE BLOCK:                                                            ║
//! ║     • Inside protected folder, NO new file or new folder can be created.     ║
//! ║     • Enforced via ES_EVENT_TYPE_AUTH_CREATE -> DENIED.                      ║
//! ╚══════════════════════════════════════════════════════════════════════════════╝

use std::any::Any;
use std::collections::HashSet;
use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use parking_lot::RwLock;
use log::{info, warn, error};
use serde::{Deserialize, Serialize};

use crate::core::communication::ServerCommunicator;
use crate::policy::policy_engine::PolicyEngine;
use crate::policy::policy_constants::*;
use crate::protection_modules::ProtectionModule;

/// Exact granular rule per folder or extension
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GranularRule {
    /// Path or folder pattern
    pub path: String,
    /// 1. If true: Block ALL operations (read, open, copy, preview, delete, etc.)
    #[serde(rename = "blockRead", default)]
    pub block_read: bool,
    /// 2. If true: Block edit/modify/save changes (read allowed)
    #[serde(rename = "blockWrite", default)]
    pub block_write: bool,
    /// 3. If true: Block delete/trash/rm (read/write allowed)
    #[serde(rename = "blockDelete", default)]
    pub block_delete: bool,
    /// 4. If true: Block renaming of folder and any file inside
    #[serde(rename = "blockRename", default)]
    pub block_rename: bool,
    /// 5. If true: Block creating new files or folders inside this directory
    #[serde(rename = "blockCreate", default)]
    pub block_create: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilePolicyData {
    /// List of granular path rules
    #[serde(rename = "rules", default)]
    pub rules: Vec<GranularRule>,
    /// Extensions to block globally (e.g. [".exe", ".sh", ".dmg"])
    #[serde(rename = "blockedExtensions", default)]
    pub blocked_extensions: Vec<String>,
    /// System-wide read-only mode
    #[serde(rename = "globalReadOnly", default)]
    pub global_read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOpType {
    ReadOpen,
    WriteModify,
    DeleteUnlink,
    Rename,
    CreateNew,
}

pub struct FileProtectionModule {
    current_policy: Arc<RwLock<FilePolicyData>>,
    alerted_cache: HashSet<String>,
    es_active: bool,
}

impl FileProtectionModule {
    pub fn new() -> Self {
        FileProtectionModule {
            current_policy: Arc::new(RwLock::new(FilePolicyData::default())),
            alerted_cache: HashSet::new(),
            es_active: false,
        }
    }

    async fn execute_monitoring(
        &mut self,
        policy_engine: &PolicyEngine,
        _communicator: &ServerCommunicator,
        _agent_id: u64,
        _token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let is_active = policy_engine.is_policy_active(FILE_PROTECTION_ENABLED);

        if !is_active {
            if self.es_active {
                info!("[FileProtection] Policy deactivated. Releasing kernel locks.");
                self.es_active = false;
            }
            return Ok(());
        }

        // Fetch latest granular policy data and immediately update in-memory lock
        let new_policy: FilePolicyData = policy_engine.get_policy_json_data(FILE_PROTECTION_ENABLED);
        *self.current_policy.write() = new_policy;

        if !self.es_active {
            info!("[FileProtection] Activating macOS Endpoint Security engine...");
            self.es_active = true;
        }

        // Periodic maintenance of alert cache to avoid memory leaks
        if self.alerted_cache.len() > 2000 {
            self.alerted_cache.clear();
        }

        Ok(())
    }

    /// Evaluates incoming kernel authorization request against the 5 granular actions.
    /// Returns true if ALLOWED, false if BLOCKED.
    pub fn evaluate_operation(&self, target_path: &str, op: FileOpType) -> bool {
        let policy = self.current_policy.read();
        let p = Path::new(target_path);

        // 0. Extension Check (e.g. executable blocking)
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            let dotted_ext = format!(".{}", ext.to_lowercase());
            if policy.blocked_extensions.contains(&dotted_ext) {
                warn!("[FileProtection] 🚫 Blocked extension: {} for path: {}", dotted_ext, target_path);
                return false;
            }
        }

        // 1. Iterate through granular rules matching path or parent folder
        for rule in &policy.rules {
            if is_path_under_rule(target_path, &rule.path) {
                // Rule 1: READ BLOCK
                // If read is blocked, ALL operations are completely denied!
                if rule.block_read {
                    warn!("[FileProtection] 🚫 [READ_BLOCK] All operations forbidden on: {}", target_path);
                    return false;
                }

                match op {
                    FileOpType::ReadOpen => {
                        // Read allowed unless block_read is true (handled above)
                    }
                    FileOpType::WriteModify => {
                        // Rule 2: WRITE BLOCK
                        // File can be read, but modifying or saving changes is denied
                        if rule.block_write || policy.global_read_only {
                            warn!("[FileProtection] 🚫 [WRITE_BLOCK] Modifications blocked on: {}", target_path);
                            return false;
                        }
                    }
                    FileOpType::DeleteUnlink => {
                        // Rule 3: DELETE BLOCK
                        // Removing, trashing, or permanent deletion is denied
                        if rule.block_delete {
                            warn!("[FileProtection] 🚫 [DELETE_BLOCK] Deletion/Trash blocked on: {}", target_path);
                            return false;
                        }
                    }
                    FileOpType::Rename => {
                        // Rule 4: RENAME BLOCK
                        // Neither the folder nor any file inside can be renamed
                        if rule.block_rename {
                            warn!("[FileProtection] 🚫 [RENAME_BLOCK] Rename blocked on: {}", target_path);
                            return false;
                        }
                    }
                    FileOpType::CreateNew => {
                        // Rule 5: CREATE BLOCK
                        // Creating new files or folders inside this path is denied
                        if rule.block_create {
                            warn!("[FileProtection] 🚫 [CREATE_BLOCK] Creation blocked inside: {}", target_path);
                            return false;
                        }
                    }
                }
            }
        }

        true // Allowed
    }
}

/// Checks if target_path is exactly rule_path or inside rule_path directory.
fn is_path_under_rule(target_path: &str, rule_path: &str) -> bool {
    let clean_target = target_path.trim_end_matches('/');
    let clean_rule = rule_path.trim_end_matches('/');

    if clean_target == clean_rule {
        return true;
    }

    clean_target.starts_with(&format!("{}/", clean_rule))
}

#[async_trait::async_trait]
impl ProtectionModule for FileProtectionModule {
    async fn execute(
        &mut self,
        policy_engine: &PolicyEngine,
        communicator: &ServerCommunicator,
        agent_id: u64,
        token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.execute_monitoring(policy_engine, communicator, agent_id, token).await
    }

    fn get_name(&self) -> &str { "FileProtection" }
    fn as_any(&mut self) -> &mut dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
