#![forbid(unsafe_code)]

//! Owned target machine code emitted from the clean terminal-Psi realization
//! lane.

use omega_target::NativeTarget;
use omega_terminal_target_operations::TerminalPsiProvenance;
use psi_core::{MachineId, OperationId};
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
    /// Typed internal-call relocation fields, ordered by `offset`. Each row
    /// points at the mutable immediate bits of one architecture-native call;
    /// object construction validates the surrounding opcode before accepting
    /// the relocation.
    pub internal_calls: Vec<TerminalInternalCallRelocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalInternalCallRelocation {
    pub psi_operation: OperationId,
    pub target: MachineId,
    /// Byte offset within this function at which the relocation field begins.
    /// On x86-64 this points at the four-byte displacement following `CALL`;
    /// on AArch64 it points at the `BL` instruction word.
    pub offset: usize,
}
