// core/setup_ui.rs
//
// Native macOS Setup & Activation Dialog for DLP Agent.
// Prompts the user or admin for:
// 1. Server URL
// 2. Username
// 3. Password
//
// Verifies connection with the central DLP backend, saves credentials to
// /Library/Application Support/DLPAgent/agent.creds, and signals the background LaunchDaemon.

use std::process::Command;
use crate::core::communication::ServerCommunicator;
use crate::core::credential_store::CredentialStore;
use crate::core::file_logger::FileLogger;

pub struct SetupParams {
    pub server_url: String,
    pub username: String,
    pub password: String,
}

pub fn run_macos_setup_wizard() {
    FileLogger::info("[SetupWizard] Launching native macOS Setup Dialog...");

    // ── AppleScript Native UI Dialogs ──────────────────────────────────────────
    // Step 1: Server URL
    let url_script = r#"
        set sUrl to text returned of (display dialog "Enter DLP Central Server URL:" ¬
            default answer "http://192.168.1.100:8080" ¬
            with title "DLP Security Agent - Step 1/3" ¬
            buttons {"Cancel", "Next"} ¬
            default button "Next" ¬
            with icon note)
        return sUrl
    "#;

    let server_url = match run_osascript(url_script) {
        Some(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => {
            FileLogger::warn("[SetupWizard] Setup cancelled by user at Server URL step.");
            return;
        }
    };

    // Step 2: Username
    let user_script = r#"
        set uName to text returned of (display dialog "Enter Agent / Employee Username:" ¬
            default answer "" ¬
            with title "DLP Security Agent - Step 2/3" ¬
            buttons {"Cancel", "Next"} ¬
            default button "Next" ¬
            with icon note)
        return uName
    "#;

    let username = match run_osascript(user_script) {
        Some(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => {
            FileLogger::warn("[SetupWizard] Setup cancelled at Username step.");
            return;
        }
    };

    // Step 3: Password (Hidden Input)
    let pass_script = r#"
        set pWord to text returned of (display dialog "Enter Agent Password:" ¬
            default answer "" ¬
            with title "DLP Security Agent - Step 3/3" ¬
            with hidden answer ¬
            buttons {"Cancel", "Activate Agent"} ¬
            default button "Activate Agent" ¬
            with icon caution)
        return pWord
    "#;

    let password = match run_osascript(pass_script) {
        Some(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            FileLogger::warn("[SetupWizard] Setup cancelled at Password step.");
            return;
        }
    };

    FileLogger::info(&format!("[SetupWizard] Connecting to server {} for user '{}'...", server_url, username));

    // Authenticate with server synchronously
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let auth_result = rt.block_on(async {
        let comm = ServerCommunicator::new(server_url.clone());
        comm.login(&username, &password).await
    });

    match auth_result {
        Ok(auth) => {
            FileLogger::info(&format!("[SetupWizard] Authentication SUCCESS! Agent ID: {}", auth.agent_id));

            // Save credentials to root-protected store
            let store = CredentialStore {
                server_url: server_url.clone(),
                agent_id: auth.agent_id,
                username: username.clone(),
                password: password.clone(),
                token: Some(auth.token),
                stop_password: None,
            };

            if let Err(e) = store.save() {
                let err_msg = format!("Failed to save credentials file: {}", e);
                FileLogger::error(&err_msg);
                show_error_dialog(&err_msg);
                return;
            }

            // Kickstart background LaunchDaemon
            let _ = Command::new("launchctl")
                .args(["kickstart", "-k", "system/com.dlpagent.daemon"])
                .output();

            // Automatically open macOS Privacy Settings pane directly (Full Disk Access)
            let _ = Command::new("open")
                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
                .spawn();

            // Show success notification and auto-close
            let success_script = format!(
                r#"display dialog "✅ DLP Agent Activated Successfully! \n\nAgent ID: {}\nServer: {}\n\nSystem Settings has opened to Full Disk Access. Please verify 'dlp-agent' is enabled." with title "Activation Complete" buttons {{"Finish"}} default button "Finish" with icon note"#,
                auth.agent_id, server_url
            );
            let _ = run_osascript(&success_script);
            FileLogger::info("[SetupWizard] Setup wizard completed successfully. Exiting UI.");
        }
        Err(e) => {
            let err_msg = format!("Login Failed: {}\n\nPlease verify Server URL and credentials.", e);
            FileLogger::error(&err_msg);
            show_error_dialog(&err_msg);
        }
    }
}

fn run_osascript(script: &str) -> Option<String> {
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn show_error_dialog(msg: &str) {
    let script = format!(
        r#"display alert "DLP Setup Error" message "{}" as critical buttons {{"OK"}}"#,
        msg.replace('"', "\\\"")
    );
    let _ = run_osascript(&script);
}
