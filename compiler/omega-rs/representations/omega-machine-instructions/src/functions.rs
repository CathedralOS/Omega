use crate::MachineInstruction;
use omega_control_flow::StateKey;
use psi_arena::HandleSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInstructionFunction {
    pub source_key: StateKey,
    pub instructions: HandleSpan<MachineInstruction>,
}

impl Default for MachineInstructionFunction {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            instructions: HandleSpan::empty(),
        }
    }
}
