// core/credential_store.rs
//
// Saves/loads agent credentials to disk so the macOS LaunchDaemon restarts
// silently without user interaction.
//
// Location: /Library/Application Support/DLPAgent/agent.creds
// Encoding: base64(XOR-obfuscated JSON)

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::core::file_logger::FileLogger;

const XOR_KEY: &[u8] = b"DLPAgent_Mac_Security_Key_2026";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialStore {
    pub server_url: String,
    pub agent_id: u64,
    pub username: String,
    pub password: String,
    pub token: Option<String>,
    pub stop_password: Option<String>,
}

impl CredentialStore {
    pub fn path() -> PathBuf {
        PathBuf::from("/Library/Application Support/DLPAgent/agent.creds")
    }

    pub fn exists() -> bool {
        Self::path().exists()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string(self)?;
        let obfuscated = xor_encrypt_bytes(json.as_bytes());
        let encoded = B64.encode(&obfuscated);

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        file.write_all(encoded.as_bytes())?;
        file.flush()?;

        // Restrict file permissions: 600 (root only)
        let _ = std::process::Command::new("chmod")
            .args(["600", &path.to_string_lossy()])
            .output();

        FileLogger::info("CredentialStore: credentials saved successfully");
        Ok(())
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::path();
        if !path.exists() {
            return Err(format!("Credential file not found at {}", path.display()).into());
        }

        let mut file = std::fs::File::open(&path)?;
        let mut encoded = String::new();
        file.read_to_string(&mut encoded)?;

        let obfuscated = B64.decode(encoded.trim())?;
        let decrypted = xor_encrypt_bytes(&obfuscated);
        let store: CredentialStore = serde_json::from_slice(&decrypted)?;

        Ok(store)
    }

    pub fn delete() -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::path();
        if path.exists() {
            std::fs::remove_file(path)?;
            FileLogger::info("CredentialStore: credentials deleted");
        }
        Ok(())
    }
}

fn xor_encrypt_bytes(data: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, &byte)| byte ^ XOR_KEY[i % XOR_KEY.len()])
        .collect()
}
