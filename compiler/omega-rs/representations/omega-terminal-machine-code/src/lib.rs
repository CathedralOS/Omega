#![forbid(unsafe_code)]

//! Owned target machine code emitted from the clean terminal-Psi realization
//! lane.

use omega_target::NativeTarget;
use omega_terminal_target_operations::{
    TerminalMetadataOnlyPortRealization, TerminalProviderExecutionBinding, TerminalPsiProvenance,
};
use psi_core::{
    BoundaryMachineId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, PlaceId, ServiceId,
};
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
    /// Target-emitter-owned stack facts for the currently supported Unit-body
    /// closure. Other terminal function forms remain deliberately unreported
    /// until their complete temporary-stack accounting is retained.
    pub unit_stack: Option<TerminalUnitStackEvidence>,
    /// Typed internal-call relocation fields, ordered by `offset`. Each row
    /// points at the mutable immediate bits of one architecture-native call;
    /// object construction validates the surrounding opcode before accepting
    /// the relocation.
    pub internal_calls: Vec<TerminalInternalCallRelocation>,
    /// Exact native byte intervals attributed to the current Psi logical-fuel
    /// schedule. These records are accounting provenance, not runtime charges.
    pub fuel_attribution: Vec<TerminalNativeFuelAttribution>,
    /// Privileged effects retained with their exact semantic service and byte
    /// range. Installation can therefore bind emitted instructions to the
    /// selected provider execution instead of inferring privilege from bytes.
    pub port_effects: Vec<TerminalPortEffectRecord>,
    /// Verified metadata-only boundary settlements retained at their exact
    /// code position. They emit no duplicate hardware effect.
    pub boundary_settlements: Vec<TerminalBoundarySettlementRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalNativeFuelSite {
    Operation(OperationId),
    Edge(EdgeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalNativeFuelAttribution {
    pub schedule: FuelScheduleIdentity,
    pub site: TerminalNativeFuelSite,
    pub units: u64,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundarySettlementRecord {
    pub psi_operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub provider_execution: TerminalProviderExecutionRecord,
    pub realization: TerminalMetadataOnlyPortRealization,
    pub argument_places: Vec<PlaceId>,
    pub claim_settlements: Vec<ClaimSettlement>,
    /// Position in the verified Unit operation sequence. This remains the
    /// canonical tie-break when multiple metadata rows share a code offset.
    pub operation_ordinal: usize,
    /// Byte offset immediately after all preceding executable operations.
    pub code_offset: usize,
}

/// Non-authoritative serialized projection of an admitted provider execution.
/// This can be decoded for validation/reporting but cannot be used to invoke
/// target lowering, which requires the ledger-owned admitted binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalProviderExecutionRecord {
    pub provider_plan: u64,
    pub provider_execution_identity: u64,
    pub provider_execution_fingerprint: u64,
    pub normalized_root_identity: u64,
    pub boundary_contract_fingerprint: u64,
}

impl TerminalProviderExecutionRecord {
    pub fn new(
        provider_plan: u64,
        provider_execution_identity: u64,
        provider_execution_fingerprint: u64,
        normalized_root_identity: u64,
        boundary_contract_fingerprint: u64,
    ) -> Option<Self> {
        [
            provider_plan,
            provider_execution_identity,
            provider_execution_fingerprint,
            normalized_root_identity,
            boundary_contract_fingerprint,
        ]
        .iter()
        .all(|identity| *identity != 0)
        .then_some(Self {
            provider_plan,
            provider_execution_identity,
            provider_execution_fingerprint,
            normalized_root_identity,
            boundary_contract_fingerprint,
        })
    }
}

impl From<TerminalProviderExecutionBinding> for TerminalProviderExecutionRecord {
    fn from(binding: TerminalProviderExecutionBinding) -> Self {
        Self {
            provider_plan: binding.provider_plan().get(),
            provider_execution_identity: binding.provider_execution_identity(),
            provider_execution_fingerprint: binding.provider_execution_fingerprint(),
            normalized_root_identity: binding.normalized_root_identity(),
            boundary_contract_fingerprint: binding.boundary_contract_fingerprint(),
        }
    }
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
    /// Exact caller-owned stack live immediately after this Unit-body call
    /// enters its callee. Absent for non-Unit function forms whose full stack
    /// accounting has not yet migrated.
    pub unit_stack: Option<TerminalUnitCallStackEvidence>,
    /// Byte offset within this function at which the relocation field begins.
    /// On x86-64 this points at the four-byte displacement following `CALL`;
    /// on AArch64 it points at the `BL` instruction word.
    pub offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalUnitStackEvidence {
    /// Maximum caller-owned bytes below this function's entry stack pointer;
    /// excludes any incoming adapter/interrupt frame and all callee-owned
    /// frames.
    pub local_peak_bytes: u32,
    pub stack_alignment: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalUnitCallStackEvidence {
    /// Function-lifetime parameter/save area still live across the call.
    pub active_frame_bytes: u32,
    /// Outgoing argument/shadow/link bytes live only while the callee runs.
    pub transient_bytes: u32,
}

impl TerminalUnitCallStackEvidence {
    pub const fn caller_live_bytes(self) -> Option<u32> {
        self.active_frame_bytes.checked_add(self.transient_bytes)
    }
}
