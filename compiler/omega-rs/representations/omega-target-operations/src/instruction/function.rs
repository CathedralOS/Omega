use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;
use std::sync::Arc;

use super::TargetOperation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationFunction {
    pub symbol: Arc<str>,
    pub source_key: StateKey,
    pub instructions: HandleSpan<TargetOperation>,
}

pub type FunctionInstructionPlan = TargetOperationFunction;

impl Default for TargetOperationFunction {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            source_key: StateKey::default(),
            instructions: HandleSpan::empty(),
        }
    }
}
