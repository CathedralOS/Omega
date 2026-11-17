//! Source obligations needed to interpret decoded CPU effects after encoding.
use selected_instructions::{SelectedCallTarget, SelectedInstructionId};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalCpuOrigins {
    pub instruction_roster: Vec<(MachineId, SelectedInstructionId)>,
    pub functions: Vec<PhysicalFunctionCpuOrigins>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFunctionCpuOrigins {
    pub machine: MachineId,
    pub unmasks_interrupts: Vec<InterruptMaskChange>,
    pub indirect_calls: Vec<(SelectedInstructionId, SelectedCallTarget)>,
    pub model_specific_registers: Vec<ModelSpecificRegisterSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptMaskChange {
    Enable(SelectedInstructionId),
    RestoreUnknownFlags(SelectedInstructionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSpecificAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSpecificRegisterSource {
    pub instruction: SelectedInstructionId,
    pub access: ModelSpecificAccess,
    pub selector: Result<Option<u64>, String>,
}

impl PhysicalCpuOrigins {
    pub fn function(&self, machine: MachineId) -> Option<&PhysicalFunctionCpuOrigins> {
        let mut rows = self.functions.iter().filter(|row| row.machine == machine);
        let row = rows.next()?;
        rows.next().is_none().then_some(row)
    }
}
