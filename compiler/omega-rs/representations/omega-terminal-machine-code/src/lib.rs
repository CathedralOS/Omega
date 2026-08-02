#![forbid(unsafe_code)]

//! Owned target machine code emitted from the clean terminal-Psi realization
//! lane.

use omega_target::NativeTarget;
use omega_terminal_target_operations::TerminalPsiProvenance;
use psi_core::MachineId;
use psi_terminal::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachineCodePlan {
    pub terminal_psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<TerminalMachineCodeFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachineCodeFunction {
    pub machine: MachineId,
    pub provenance: TerminalPsiProvenance,
    pub bytes: Vec<u8>,
}
