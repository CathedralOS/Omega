use omega_control_flow::MachineFunctionIdentity;
use psi_arena::HandleSpan;
use std::sync::Arc;

use super::AbstractOperation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractFunctionPlan {
    pub symbol: Arc<str>,
    pub identity: MachineFunctionIdentity,
    pub instructions: HandleSpan<AbstractOperation>,
}

pub type FunctionInstructionPlan = AbstractFunctionPlan;

impl Default for AbstractFunctionPlan {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            identity: MachineFunctionIdentity::default(),
            instructions: HandleSpan::empty(),
        }
    }
}
