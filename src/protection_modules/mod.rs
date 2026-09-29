//! protection_modules/mod.rs — Protection Module trait + sub-modules

pub mod usb_protection;
pub mod web_filter;
pub mod file_protection;
pub mod security_monitor;
pub mod app_usage;

use std::any::Any;
use std::error::Error;

use crate::core::communication::ServerCommunicator;
use crate::policy::policy_engine::PolicyEngine;

/// Core trait every protection module must implement.
#[async_trait::async_trait]
pub trait ProtectionModule: Send + Sync {
    async fn execute(
        &mut self,
        policy_engine: &PolicyEngine,
        communicator: &ServerCommunicator,
        agent_id: u64,
        token: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    fn get_name(&self) -> &str;
    fn as_any(&mut self) -> &mut dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
