//! Explicit binding seam. No provider is selected implicitly and no delegated request runs in V1.
use crate::{CoreError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    DelegateTask,
    GenerateImage,
    AnalyzeImage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityBinding {
    pub capability: Capability,
    pub provider: String,
    pub account_profile_id: String,
}

pub struct DelegationLimits {
    pub max_depth: u8,
    pub max_calls_per_turn: u16,
}
impl Default for DelegationLimits {
    fn default() -> Self {
        Self {
            max_depth: 2,
            max_calls_per_turn: 4,
        }
    }
}
impl DelegationLimits {
    pub fn validate(&self, depth: u8, calls: u16) -> Result<()> {
        if depth >= self.max_depth || calls >= self.max_calls_per_turn {
            return Err(CoreError::Invalid("Достигнут лимит делегирования".into()));
        }
        Ok(())
    }
}

pub fn require_binding(binding: Option<&CapabilityBinding>) -> Result<&CapabilityBinding> {
    binding.ok_or(CoreError::CapabilityUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_implicit_provider_and_recursion_is_bounded() {
        assert!(require_binding(None).is_err());
        assert!(DelegationLimits::default().validate(2, 0).is_err());
        assert!(DelegationLimits::default().validate(0, 4).is_err());
        assert!(DelegationLimits::default().validate(1, 3).is_ok());
    }
}
