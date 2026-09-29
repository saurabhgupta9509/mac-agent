// core/config.rs
// Central configuration for sensitive-data detection patterns and settings.

use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;

lazy_static! {
    pub static ref PATTERNS: HashMap<&'static str, Regex> = {
        let mut m = HashMap::new();
        m.insert("card_16", Regex::new(r"\b[0-9OoDIl|!SszZBG]{4}[- ]?[0-9OoDIl|!SszZBG]{4}[- ]?[0-9OoDIl|!SszZBG]{4}[- ]?[0-9OoDIl|!SszZBG]{4}\b").unwrap());
        m.insert("card_15", Regex::new(r"\b[0-9OoDIl|!SszZBG]{4}[- ]?[0-9OoDIl|!SszZBG]{6}[- ]?[0-9OoDIl|!SszZBG]{5}\b").unwrap());
        m.insert("phone_india", Regex::new(r"\b(\+91[\s-]?)?[6789]\d{9}\b").unwrap());
        m.insert("phone_with_country", Regex::new(r"\+\d{1,3}[- ]?\d{3}[- ]?\d{3}[- ]?\d{4}\b").unwrap());
        m.insert("credentials", Regex::new(r"(?i)\b(password|pwd|passcode|login|credential)\b").unwrap());
        m.insert("keys_tokens", Regex::new(r"(?i)\b(api[_ ]?key|secret|token|auth|bearer)\b").unwrap());
        m.insert("email", Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap());
        m.insert("ssn", Regex::new(r"\b\d{3}[- ]?\d{2}[- ]?\d{4}\b").unwrap());
        m.insert("aadhaar", Regex::new(r"\b[0-9OoDIl|!SszZBG]{4}[\s-]?[0-9OoDIl|!SszZBG]{4}[\s-]?[0-9OoDIl|!SszZBG]{4}\b").unwrap());
        m.insert("pan_card", Regex::new(r"(?i)\b[A-Z]{5}[0-9]{4}[A-Z]{1}\b").unwrap());
        m.insert("jwt", Regex::new(r"\beyJ[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+\.[A-Za-z0-9-_+/=]+\b").unwrap());
        m.insert("api_key", Regex::new(r"\b[A-Za-z0-9-_]{32,64}\b").unwrap());
        m
    };

    pub static ref FALSE_POSITIVE_PATTERNS: HashMap<&'static str, Regex> = {
        let mut m = HashMap::new();
        m.insert("test_credit_cards", Regex::new(r"(?i)(4111[- ]?1111[- ]?1111[- ]?1111|5555[- ]?5555[- ]?5555[- ]?4444|3782[- ]?822463[- ]?10005|4242[- ]?4242[- ]?4242[- ]?4242)").unwrap());
        m.insert("sequential_numbers", Regex::new(r"\b(1234[- ]?1234[- ]?1234|1111[- ]?1111[- ]?1111|0000[- ]?0000[- ]?0000|9999[- ]?9999[- ]?9999|2222[- ]?2222[- ]?2222|3333[- ]?3333[- ]?3333)\b").unwrap());
        m.insert("example_domains", Regex::new(r"@(example\.com|test\.com|domain\.com|company\.com|sample\.org|email\.com|fake\.com|dummy\.com)").unwrap());
        m.insert("documentation_keywords", Regex::new(r"(?i)\b(example|sample|test|demo|documentation|placeholder|fictional|dummy|fake|mock|lorem|ipsum)\b").unwrap());
        m.insert("file_path_patterns", Regex::new(r"(?i)\b(?:screenshot_\d{8}_\d{6}\.png|logs?/|screenshots?/|certificates?/|\d{4}[-_]\d{2}[-_]\d{2}|[\w/.-]+\.(?:png|jpg|jpeg|json|log|txt|py|js|html?|css|xml|yaml|yml)|dlp[-_]?agent[-_][\w-]{5,}|dlpagent[-_][\w-]{5,})\b").unwrap());
        m
    };
}

pub const OCR_INTERVAL: u64 = 5;
pub const LOG_RETENTION: u64 = 60;

pub fn get_base_url() -> String {
    // 1. Try to load from credential store
    if let Ok(store) = crate::core::credential_store::CredentialStore::load() {
        if !store.server_url.trim().is_empty() {
            return store.server_url.trim().trim_end_matches('/').to_string();
        }
    }

    // 2. Fallback to Environment Variable
    if let Ok(env_url) = std::env::var("DLP_SERVER_URL") {
        return env_url.trim().trim_end_matches('/').to_string();
    }

    "http://127.0.0.1:8080".to_string()
}
