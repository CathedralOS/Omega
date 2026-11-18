//! Source obligations for independent installed-handler fault readback.
use selected_instructions::{SelectedCallTarget, SelectedInstructionId};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFaultOrigins {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub entry: MachineId,
    pub target: target::NativeTarget,
    pub physical_identity: [u8; 32],
    pub functions: Vec<PhysicalFunctionFaultOrigins>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFunctionFaultOrigins {
    pub machine: MachineId,
    pub instructions: Vec<PhysicalInstructionFaultOrigin>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalInstructionFaultOrigin {
    pub instruction: SelectedInstructionId,
    pub operation: FaultOperation,
    pub description: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultOperation {
    MemoryOrStack,
    Privileged,
    Call(SelectedCallTarget),
    Other,
}
