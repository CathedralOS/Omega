use omega_control_flow::MachineFunctionIdentity;
use psi_arena::HandleSpan;
use std::sync::Arc;

use super::TargetOperation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationFunction {
    pub symbol: Arc<str>,
    pub identity: MachineFunctionIdentity,
    pub instructions: HandleSpan<TargetOperation>,
}

pub type FunctionInstructionPlan = TargetOperationFunction;

impl Default for TargetOperationFunction {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            identity: MachineFunctionIdentity::default(),
            instructions: HandleSpan::empty(),
        }
    }
}
