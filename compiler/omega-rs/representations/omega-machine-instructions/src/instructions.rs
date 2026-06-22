use crate::MachineInstructionKind;
use omega_assigned_target_operations::SelectedInstructionKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInstruction {
    pub selected_instruction_index: u32,
    pub source_kind: SelectedInstructionKind,
    pub kind: MachineInstructionKind,
}

impl Default for MachineInstruction {
    fn default() -> Self {
        Self {
            selected_instruction_index: 0,
            source_kind: SelectedInstructionKind::EnterFunction,
            kind: MachineInstructionKind::NoOp,
        }
    }
}
