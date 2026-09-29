//! lib.rs — Mac DLP Agent Library Root
//!
//! Re-exports core modules and protection modules for macOS.

pub mod core;
pub mod policy;
pub mod protection_modules;
pub mod platform;

// Re-export core types
pub use core::agent_core::AgentCore;
pub use core::credential_store;
pub use core::file_logger;
pub use core::network_utils;
pub use core::communication::ServerCommunicator;
pub use policy::policy_engine::PolicyEngine;
pub use protection_modules::ProtectionModule;
