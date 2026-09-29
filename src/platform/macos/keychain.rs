//! platform/macos/keychain.rs
//!
//! macOS Keychain integration.
//! Replaces Windows Credential Manager / winreg credential storage.
//!
//! Uses Security.framework (SecItem APIs).

use crate::core::file_logger::FileLogger;

const SERVICE_NAME: &str = "com.dlpagent.credentials";

/// Store a credential in the macOS Keychain.
/// Equivalent to: Windows CredWrite() or Registry-based credential store.
pub fn store_credential(account: &str, password: &str) -> Result<(), String> {
    FileLogger::info(&format!("[Keychain] Storing credential for account: {}", account));

    #[cfg(target_os = "macos")]
    {
        use security_framework::passwords::set_generic_password;
        set_generic_password(SERVICE_NAME, account, password.as_bytes())
            .map_err(|e| format!("Keychain store error: {}", e))?;
    }

    Ok(())
}

/// Retrieve a credential from the macOS Keychain.
/// Equivalent to: Windows CredRead()
pub fn get_credential(account: &str) -> Result<String, String> {
    FileLogger::info(&format!("[Keychain] Getting credential for account: {}", account));

    #[cfg(target_os = "macos")]
    {
        use security_framework::passwords::get_generic_password;
        let bytes = get_generic_password(SERVICE_NAME, account)
            .map_err(|e| format!("Keychain get error: {}", e))?;
        return String::from_utf8(bytes).map_err(|e| e.to_string());
    }

    #[cfg(not(target_os = "macos"))]
    Err("Keychain only available on macOS".to_string())
}

/// Delete a credential from the macOS Keychain.
pub fn delete_credential(account: &str) -> Result<(), String> {
    FileLogger::info(&format!("[Keychain] Deleting credential for account: {}", account));

    #[cfg(target_os = "macos")]
    {
        use security_framework::passwords::delete_generic_password;
        delete_generic_password(SERVICE_NAME, account)
            .map_err(|e| format!("Keychain delete error: {}", e))?;
    }

    Ok(())
}
