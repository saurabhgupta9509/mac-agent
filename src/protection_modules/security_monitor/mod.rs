//! protection_modules/security_monitor/mod.rs + monitor.rs
//!
//! ╔══════════════════════════════════════════════════════════════════╗
//! ║       macOS Security Monitor — OCR + LLM Threat Analysis        ║
//! ║                                                                  ║
//! ║  HOW IT WORKS:                                                   ║
//! ║  ┌───────────────────────────────────────────────────────┐      ║
//! ║  │  Every 5 seconds (configurable):                      │      ║
//! ║  │                                                       │      ║
//! ║  │  STEP 1: Screenshot                                   │      ║
//! ║  │    CGDisplayCreateImage(CGMainDisplayID())            │      ║
//! ║  │    → CGImage → PNG bytes                              │      ║
//! ║  │    (Windows: screenshots crate / BitBlt)             │      ║
//! ║  │                                                       │      ║
//! ║  │  STEP 2: OCR                                          │      ║
//! ║  │    Option A: Apple Vision Framework                   │      ║
//! ║  │      VNImageRequestHandler +                          │      ║
//! ║  │      VNRecognizeTextRequest                           │      ║
//! ║  │    Option B: Tesseract (same as Windows/Linux)        │      ║
//! ║  │    → Extracts all text from screenshot               │      ║
//! ║  │    (Windows: Tesseract / paddle-ocr)                 │      ║
//! ║  │                                                       │      ║
//! ║  │  STEP 3: Rule Engine                                  │      ║
//! ║  │    Regex patterns from server policy                  │      ║
//! ║  │    Match: credit cards, emails, phone, SSN, etc.     │      ║
//! ║  │    (Same rule engine — 100% portable Rust)           │      ║
//! ║  │                                                       │      ║
//! ║  │  STEP 4: LLM Threat Assessment (if violation found)  │      ║
//! ║  │    POST → server LLM API with context text           │      ║
//! ║  │    → Severity score + threat classification           │      ║
//! ║  │    (Same REST call — 100% portable)                  │      ║
//! ║  │                                                       │      ║
//! ║  │  STEP 5: Alert + Certificate                         │      ║
//! ║  │    Send violation to server                           │      ║
//! ║  │    Generate PDF certificate (if configured)          │      ║
//! ║  └───────────────────────────────────────────────────────┘      ║
//! ║                                                                  ║
//! ║  Entitlement needed: com.apple.security.screen-recording         ║
//! ║  (User must grant Screen Recording permission in               ║
//! ║   System Settings → Privacy & Security → Screen Recording)     ║
//! ╚══════════════════════════════════════════════════════════════════╝

pub mod monitor;
pub mod ocr;
pub mod image_processing;
pub mod rule_engine;
pub mod llm_threat_assessor;
pub mod entity_engine;
pub mod types;

pub use monitor::SecurityMonitorModule;
