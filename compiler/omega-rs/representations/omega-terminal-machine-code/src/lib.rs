#![forbid(unsafe_code)]

//! Owned target machine code emitted from the clean terminal-Psi realization
//! lane.

use omega_target::NativeTarget;
use omega_terminal_target_operations::{
    TerminalMetadataOnlyPortRealization, TerminalProviderExecutionBinding, TerminalPsiProvenance,
};
use psi_core::{BoundaryMachineId, MachineId, OperationId, PlaceId, ServiceId};
use psi_terminal::{ClaimSettlement, TerminalPsiIdentity};

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
    /// Privileged effects retained with their exact semantic service and byte
    /// range. Installation can therefore bind emitted instructions to the
    /// selected provider execution instead of inferring privilege from bytes.
    pub port_effects: Vec<TerminalPortEffectRecord>,
    /// Verified metadata-only boundary settlements retained at their exact
    /// code position. They emit no duplicate hardware effect.
    pub boundary_settlements: Vec<TerminalBoundarySettlementRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundarySettlementRecord {
    pub psi_operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub provider_execution: TerminalProviderExecutionBinding,
    pub realization: TerminalMetadataOnlyPortRealization,
    pub argument_places: Vec<PlaceId>,
    pub claim_settlements: Vec<ClaimSettlement>,
    /// Position in the verified Unit operation sequence. This remains the
    /// canonical tie-break when multiple metadata rows share a code offset.
    pub operation_ordinal: usize,
    /// Byte offset immediately after all preceding executable operations.
    pub code_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPortEffectRecord {
    pub psi_operation: OperationId,
    pub service: ServiceId,
    pub port: u16,
    pub value: u8,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
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
