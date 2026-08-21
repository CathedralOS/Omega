use crate::MachineInstruction;
use omega_control_flow::MachineFunctionIdentity;
use psi_arena::HandleSpan;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInstructionFunction {
    pub symbol: Arc<str>,
    pub identity: MachineFunctionIdentity,
    pub instructions: HandleSpan<MachineInstruction>,
}

impl Default for MachineInstructionFunction {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            identity: MachineFunctionIdentity::default(),
            instructions: HandleSpan::empty(),
        }
    }
}
