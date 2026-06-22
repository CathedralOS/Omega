use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;
use std::sync::Arc;

use super::AbstractOperation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractFunctionPlan {
    pub symbol: Arc<str>,
    pub source_key: StateKey,
    pub instructions: HandleSpan<AbstractOperation>,
}

pub type FunctionInstructionPlan = AbstractFunctionPlan;

impl Default for AbstractFunctionPlan {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            source_key: StateKey::default(),
            instructions: HandleSpan::empty(),
        }
    }
}
