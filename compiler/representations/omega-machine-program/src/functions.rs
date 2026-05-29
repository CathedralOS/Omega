use crate::MachineInstruction;
use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFunction {
    pub source_key: StateKey,
    pub instructions: HandleSpan<MachineInstruction>,
}

impl Default for MachineFunction {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            instructions: HandleSpan::empty(),
        }
    }
}
