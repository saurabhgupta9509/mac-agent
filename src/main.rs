// main.rs — Mac DLP Agent Entry Point
//
// macOS equivalent of the Windows Service entry point.
// Runs as a LaunchDaemon (root) or with --ui-helper flag
// to run in user session context.
//
// Architecture:
//   LaunchDaemon  ←→  dlp-agent (root process)
//   LaunchAgent   ←→  dlp-agent --ui-helper (user process)

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dlp_agent_mac::core::service_runner;
use dlp_agent_mac::file_logger::FileLogger;

fn main() {
    // Initialize file logger
    FileLogger::init("/var/log/dlp-agent/agent.log");

    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--ui-helper".to_string()) {
        // User-session mode: launched by LaunchAgent
        FileLogger::info("Starting in UI Helper (user session) mode");
        run_ui_helper();
    } else if args.contains(&"--verify-stop".to_string()) {
        // Show stop-verification UI to user
        FileLogger::info("Launching stop verification UI");
        run_stop_verification_ui();
    } else if args.contains(&"--verify-start".to_string()) {
        // Show start-verification UI to user
        FileLogger::info("Launching start verification UI");
        run_start_verification_ui();
    } else if args.contains(&"--setup".to_string()) {
        // Setup wizard mode: prompts Server URL, Username, Password and activates agent
        FileLogger::info("Launching DLP Setup & Activation Wizard");
        dlp_agent_mac::core::setup_ui::run_macos_setup_wizard();
    } else {
        // Default: run as LaunchDaemon (root-level service)
        FileLogger::info("Starting DLP Agent daemon (macOS LaunchDaemon mode)");
        run_daemon();
    }
}

/// Main daemon entry — runs as root via LaunchDaemon.
fn run_daemon() {
    // Check for admin stop flag
    if service_runner::is_service_stopped_by_admin() {
        let auth_file = "/Library/Application Support/DLPAgent/start_authorized";
        if std::path::Path::new(auth_file).exists() {
            FileLogger::info("Admin approved restart. Resuming...");
            let _ = std::fs::remove_file(auth_file);
        } else {
            FileLogger::info("Service stopped by admin. Waiting for authorization...");
            launch_start_verification_ui();

            // Wait up to 300 seconds for authorization
            let mut approved = false;
            for _ in 0..300 {
                if std::path::Path::new(auth_file).exists() {
                    approved = true;
                    break;
                }
                std::thread::sleep(Duration::from_secs(1));
            }

            if !approved {
                FileLogger::warn("Start authorization timed out. Exiting.");
                std::process::exit(1);
            }
            let _ = std::fs::remove_file(auth_file);
        }
    }

    // Clean stale auth files
    let _ = std::fs::remove_file("/Library/Application Support/DLPAgent/stop_authorized");
    let _ = std::fs::remove_file("/Library/Application Support/DLPAgent/start_authorized");

    // Create required directories
    let _ = std::fs::create_dir_all("/Library/Application Support/DLPAgent");
    let _ = std::fs::create_dir_all("/var/log/dlp-agent");

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // Setup SIGTERM handler (equivalent to Windows SCM Stop)
    ctrlc::set_handler(move || {
        let auth_file = "/Library/Application Support/DLPAgent/stop_authorized";
        if std::path::Path::new(auth_file).exists() {
            FileLogger::info("SIGTERM received — stop authorized. Shutting down...");
            running_clone.store(false, Ordering::SeqCst);
        } else {
            FileLogger::info("SIGTERM received — launching stop verification UI...");
            launch_stop_verification_ui();
        }
    })
    .expect("Error setting Ctrl-C handler");

    FileLogger::info("DLP Agent daemon running — starting Tokio runtime");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .thread_name("dlp-worker")
        .build()
        .expect("Failed to build Tokio runtime");

    rt.block_on(async {
        service_runner::run_agent_until_stopped(running.clone()).await;
    });

    FileLogger::info("DLP Agent daemon stopped.");
    std::process::exit(0);
}

/// Runs in user session — monitors apps, clipboard, screen.
fn run_ui_helper() {
    FileLogger::info("Starting UI Helper in user session");

    // Automatically prompt Setup Wizard if agent is not yet activated
    let creds_file = "/Library/Application Support/DLPAgent/agent.creds";
    if !std::path::Path::new(creds_file).exists() {
        FileLogger::info("No agent.creds found — launching Setup Wizard directly in active user GUI session!");
        dlp_agent_mac::core::setup_ui::run_macos_setup_wizard();
    }

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    ctrlc::set_handler(move || {
        running_clone.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .expect("Failed to build runtime");

    rt.block_on(async {
        FileLogger::info("UI Helper: user-session monitors starting...");
        while running.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

fn launch_stop_verification_ui() {
    let app_path = "/Applications/DLPAgent.app/Contents/MacOS/dlp-ui";
    let _ = std::process::Command::new(app_path)
        .arg("--verify-stop")
        .spawn();
}

fn launch_start_verification_ui() {
    let app_path = "/Applications/DLPAgent.app/Contents/MacOS/dlp-ui";
    let _ = std::process::Command::new(app_path)
        .arg("--verify-start")
        .spawn();
}

fn run_stop_verification_ui() {
    FileLogger::info("Stop verification UI placeholder");
}

fn run_start_verification_ui() {
    FileLogger::info("Start verification UI placeholder");
}
