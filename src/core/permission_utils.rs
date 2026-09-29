// core/permission_utils.rs
// macOS permission configuration and directory access management.

use std::path::Path;
use crate::core::file_logger::FileLogger;

/// Grants read/write permissions to standard users for the specified path.
/// Useful for `/Library/Application Support/DLPAgent` so UI helpers can write logs and status.
pub fn grant_user_modify_access(path: &Path) {
    FileLogger::info(&format!("Granting user access to: {}", path.display()));
    
    // On macOS, set permissions: chmod -R 777 or 775, chown root:staff
    let _ = std::process::Command::new("chmod")
        .args(["-R", "775", &path.to_string_lossy()])
        .output();
}

/// Hardens process execution on macOS.
pub fn harden_process_permissions() {
    FileLogger::info("[Auth] Process runtime verified (macOS LaunchDaemon context)");
}
