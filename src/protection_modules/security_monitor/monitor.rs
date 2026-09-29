//! protection_modules/security_monitor/monitor.rs
//!
//! macOS Security Monitor — screen capture + OCR + LLM threat detection.

use std::any::Any;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use log::{info, warn, error};
use serde::{Deserialize, Serialize};
use base64::{engine::general_purpose, Engine as _};

use crate::core::communication::ServerCommunicator;
use crate::policy::policy_engine::PolicyEngine;
use crate::protection_modules::ProtectionModule;

const SCREENSHOT_INTERVAL_SECS: u64 = 5;
const SCREENSHOTS_DIR: &str = "/Library/Application Support/DLPAgent/screenshots";

pub struct SecurityMonitorModule {
    pub agent_id: u64,
    pub last_screenshot_time: u64,
    pub violation_count: u32,
    pub screenshots_dir: PathBuf,
}

impl SecurityMonitorModule {
    pub fn new(agent_id: u64) -> Self {
        let dir = PathBuf::from(SCREENSHOTS_DIR);
        let _ = std::fs::create_dir_all(&dir);
        SecurityMonitorModule {
            agent_id,
            last_screenshot_time: 0,
            violation_count: 0,
            screenshots_dir: dir,
        }
    }

    async fn execute_monitoring(
        &mut self,
        policy_engine: &PolicyEngine,
        communicator: &ServerCommunicator,
        agent_id: u64,
        token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {

        if !policy_engine.is_policy_active("POLICY_OCR_MONITOR") {
            return Ok(());
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

        if now - self.last_screenshot_time < SCREENSHOT_INTERVAL_SECS {
            return Ok(());
        }
        self.last_screenshot_time = now;

        // ── STEP 1: Capture Screenshot ─────────────────────────────────────
        info!("[SecurityMonitor] Capturing screen...");
        let screenshot_path = self.screenshots_dir
            .join(format!("screen_{}.png", now));

        let captured = capture_screenshot_macos(&screenshot_path)?;
        if !captured {
            return Ok(());
        }

        // ── STEP 2: OCR ───────────────────────────────────────────────────
        info!("[SecurityMonitor] Running OCR...");
        let ocr_text = run_ocr_on_image(&screenshot_path);
        if ocr_text.trim().is_empty() {
            let _ = std::fs::remove_file(&screenshot_path);
            return Ok(());
        }

        // ── STEP 3: Rule Engine ───────────────────────────────────────────
        let violations = scan_text_for_violations(&ocr_text);
        if violations.is_empty() {
            let _ = std::fs::remove_file(&screenshot_path);
            return Ok(());
        }

        // ── STEP 4: Build alert payload ────────────────────────────────────
        warn!("[SecurityMonitor] 🚨 Violations detected: {:?}", violations);
        self.violation_count += violations.len() as u32;

        // Encode screenshot as base64 for server
        let img_bytes = std::fs::read(&screenshot_path).unwrap_or_default();
        let img_b64 = general_purpose::STANDARD.encode(&img_bytes);

        let payload = serde_json::json!({
            "agentId": agent_id,
            "alertType": "OCR_VIOLATION",
            "violations": violations,
            "ocrText": &ocr_text[..ocr_text.len().min(2000)], // Truncate
            "screenshotBase64": img_b64,
            "timestamp": now,
            "platform": "macOS",
        });

        communicator.send_alert(agent_id, token, &payload).await?;
        info!("[SecurityMonitor] Alert sent for {} violations.", violations.len());

        // Cleanup
        let _ = std::fs::remove_file(&screenshot_path);
        Ok(())
    }
}

// ─── macOS Screenshot ─────────────────────────────────────────────────────────
/// Capture the screen using `screencapture` CLI (wraps Core Graphics).
///
/// macOS native approach:
///   CGDisplayCreateImage(CGMainDisplayID()) → write as PNG
///
/// We use the `screencapture` CLI as a reliable fallback that wraps
/// the same Core Graphics API — no entitlement needed for this method.
///
/// For the full native approach (CGDisplayCreateImage in Rust), we'd use
/// the `core-graphics` crate:
///   let display = CGDisplay::main();
///   let image = display.image().unwrap();
///   // convert CGImage to PNG...
///
/// Windows equivalent: screenshots crate (BitBlt / PrintWindow)
fn capture_screenshot_macos(output_path: &PathBuf) -> Result<bool, Box<dyn Error + Send + Sync>> {
    // Method 1: screencapture CLI (most reliable on macOS)
    let result = Command::new("screencapture")
        .args(&[
            "-x",                                    // No sound
            "-t", "png",                             // PNG format
            output_path.to_str().unwrap_or(""),      // Output path
        ])
        .output()?;

    if result.status.success() && output_path.exists() {
        info!("[SecurityMonitor] Screenshot captured: {}", output_path.display());
        return Ok(true);
    }

    // Method 2: Using core-graphics crate (production approach)
    // Requires Screen Recording permission from user
    // let display_id = unsafe { CGMainDisplayID() };
    // let image = unsafe { CGDisplayCreateImage(display_id) };
    // ... encode to PNG and write to output_path ...

    error!("[SecurityMonitor] Screenshot capture failed.");
    Ok(false)
}

// ─── OCR ──────────────────────────────────────────────────────────────────────
/// Extract text from image using Tesseract CLI or Apple Vision.
///
/// Method 1: Tesseract CLI (same as Windows/Linux — portable)
///   Requires: brew install tesseract
///
/// Method 2: Apple Vision Framework (native macOS)
///   VNImageRequestHandler + VNRecognizeTextRequest
///   Best accuracy, no external dependency
///   Via objc2-vision crate in production
///
/// Windows equivalent: Tesseract / paddle-ocr (ocr-rs crate)
fn run_ocr_on_image(image_path: &PathBuf) -> String {
    // Method 1: Tesseract CLI
    let output = Command::new("tesseract")
        .args(&[
            image_path.to_str().unwrap_or(""),
            "stdout",           // Output to stdout
            "--psm", "1",       // Automatic page segmentation
            "--oem", "3",       // LSTM + legacy engine
        ])
        .output();

    if let Ok(o) = output {
        if o.status.success() {
            return String::from_utf8_lossy(&o.stdout).to_string();
        }
    }

    // Method 2: Apple Vision via swift subprocess (fallback)
    // In production: use objc2-vision:
    //   let request = VNRecognizeTextRequest::new(completion_handler);
    //   request.setRecognitionLevel(VNRequestTextRecognitionLevelAccurate);
    //   let handler = VNImageRequestHandler::new_with_url(image_url, options);
    //   handler.perform(&[request]);

    String::new()
}

// ─── Rule Engine ──────────────────────────────────────────────────────────────
/// Scan OCR text for policy violations using regex patterns.
/// 100% portable — identical to Windows/Linux agent rule engine.
fn scan_text_for_violations(text: &str) -> Vec<serde_json::Value> {
    use regex::Regex;
    let mut violations = Vec::new();

    let patterns: &[(&str, &str)] = &[
        (r"\b\d{4}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b", "CREDIT_CARD"),
        (r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b", "EMAIL"),
        (r"\b\d{3}[-.\s]?\d{2}[-.\s]?\d{4}\b", "SSN"),
        (r"\b\+?[1-9]\d{9,14}\b", "PHONE_NUMBER"),
        (r"(?i)(password|passwd|secret|api_key|token)\s*[:=]\s*\S+", "CREDENTIAL"),
        (r"(?i)(confidential|secret|top secret|restricted|private)", "SENSITIVE_LABEL"),
    ];

    for (pattern, violation_type) in patterns {
        if let Ok(re) = Regex::new(pattern) {
            if re.is_match(text) {
                let matches: Vec<&str> = re.find_iter(text)
                    .take(3)
                    .map(|m| m.as_str())
                    .collect();
                violations.push(serde_json::json!({
                    "type": violation_type,
                    "matches": matches,
                    "severity": get_severity(violation_type),
                }));
            }
        }
    }

    violations
}

fn get_severity(vtype: &str) -> &str {
    match vtype {
        "CREDIT_CARD" | "SSN" | "CREDENTIAL" => "HIGH",
        "EMAIL" | "PHONE_NUMBER" => "MEDIUM",
        _ => "LOW",
    }
}

// ─── ProtectionModule Trait ───────────────────────────────────────────────────
#[async_trait::async_trait]
impl ProtectionModule for SecurityMonitorModule {
    async fn execute(
        &mut self,
        policy_engine: &PolicyEngine,
        communicator: &ServerCommunicator,
        agent_id: u64,
        token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.execute_monitoring(policy_engine, communicator, agent_id, token).await
    }

    fn get_name(&self) -> &str { "SecurityMonitor" }
    fn as_any(&mut self) -> &mut dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
