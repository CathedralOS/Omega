//! Function resource bounds carried from physical planning.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionTemporaryStack {
    pub machine: semantic_vocabulary::MachineId,
    pub byte_count: u32,
}

/// One physical port event and the exact source occurrence it publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalPortWrite {
    pub machine: semantic_vocabulary::MachineId,
    pub instruction: selected_instructions::SelectedInstructionId,
    pub operation: semantic_vocabulary::OperationId,
    pub service: semantic_vocabulary::ServiceId,
    pub port: u16,
    pub value: u8,
}
