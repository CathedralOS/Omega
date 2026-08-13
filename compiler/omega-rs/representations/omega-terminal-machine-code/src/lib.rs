#![forbid(unsafe_code)]

//! Owned target machine code emitted from the clean terminal-Psi realization
//! lane.

use omega_calling_conventions::{ValuePlacement, ValueShape};
use omega_target::NativeTarget;
use omega_terminal_target_operations::{
    TerminalMetadataOnlyPortRealization, TerminalProviderExecutionBinding, TerminalPsiProvenance,
};
use psi_core::{
    BoundaryMachineId, ClaimId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, PlaceId,
    ServiceId,
};
use psi_terminal::{
    CompletionReceipt, StructuralArgument, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    TerminalPsiIdentity,
};

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
    /// Exact ordered stack mutations and admitted control-flow shape for a
    /// scalar function. Object construction replays the target instructions
    /// and derives the numeric peak; unsupported scalar forms remain `None`.
    pub scalar_stack: Option<TerminalScalarStackEvidence>,
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
    /// Exact ownership-bearing structural return bound to the emitted byte
    /// interval. Claim identities are semantic metadata, never hidden ABI
    /// words.
    pub structural_return: Option<TerminalStructuralReturnRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralReturnRecord {
    pub psi_edge: EdgeId,
    /// Complete ordered structural input signature. This binds the returned
    /// place and every zero-code cleanup place to its exact type/multiplicity.
    pub parameters: Vec<StructuralParameterDeclaration>,
    /// Complete ABI input placement list corresponding one-for-one with
    /// `parameters`, including cleanup-only inputs.
    pub parameter_placements: Vec<ValuePlacement>,
    pub source: StructuralParameterDeclaration,
    pub result: StructuralResultDeclaration,
    pub shape: ValueShape,
    pub source_placement: ValuePlacement,
    pub result_placement: ValuePlacement,
    pub returned_claims: Vec<ClaimId>,
    /// Typed no-ABI local declarations. These never receive a placement and
    /// therefore cannot silently become runtime inputs.
    pub trivial_affine_locals: Vec<(
        OperationId,
        StructuralPlaceDeclaration,
        StructuralTypeDeclaration,
    )>,
    /// Exact verifier-owned reverse-declaration no-code cleanup order.
    pub trivial_affine_discards: Vec<PlaceId>,
    pub code_offset: usize,
    pub byte_count: usize,
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
    /// Exact typed Psi custody arguments, including structural projections.
    /// These are provider-settlement evidence and do not describe an internal
    /// Unit-call ABI.
    pub arguments: Vec<StructuralArgument>,
    pub completion_receipts: Vec<CompletionReceipt>,
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
    /// Exact scalar-expression call-site stack facts. The object boundary
    /// replays the surrounding ordered mutations and derives caller-live
    /// bytes; this evidence names only the encoded outbound area and, on
    /// AArch64, the explicit link-register save/restore.
    pub scalar_stack: Option<TerminalScalarCallStackEvidence>,
    /// Byte offset within this function at which the relocation field begins.
    /// On x86-64 this points at the four-byte displacement following `CALL`;
    /// on AArch64 it points at the `BL` instruction word.
    pub offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalUnitStackEvidence {
    /// Exact function-lifetime stack allocation and matching release. The
    /// object boundary validates both target encodings before deriving any
    /// numeric stack demand. `None` is valid only for an x86-64 Unit leaf with
    /// no parameter-home frame.
    pub frame: Option<TerminalStackAdjustmentPair>,
    /// AArch64 Unit functions retain the incoming link register in their
    /// function-lifetime frame. Both accesses are validated against the exact
    /// encoded instructions; x86-64 uses the implicit CALL/RET stack link.
    pub aarch64_return_link: Option<TerminalAarch64ReturnLinkEvidence>,
    pub stack_alignment: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalUnitCallStackEvidence {
    /// Exact outgoing argument/shadow allocation and matching release around
    /// this call. The object boundary derives the transient contribution from
    /// these validated target instructions plus architecture-owned linkage.
    pub outbound: Option<TerminalStackAdjustmentPair>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalScalarCallStackEvidence {
    pub outbound: Option<TerminalStackAdjustmentPair>,
    pub aarch64_return_link: Option<TerminalAarch64ReturnLinkEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalStackAdjustmentPair {
    pub byte_size: u32,
    pub allocation_offset: usize,
    pub allocation_byte_count: usize,
    pub release_offset: usize,
    pub release_byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalAarch64ReturnLinkEvidence {
    pub frame_byte_offset: u32,
    pub store_offset: usize,
    pub load_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarStackEvidence {
    pub mutations: Vec<TerminalScalarStackMutation>,
    pub control_flow: TerminalScalarControlFlowEvidence,
    pub stack_alignment: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalScalarControlFlowEvidence {
    Linear,
    /// One top-level Boolean branch followed by two independently returning
    /// linear integer arms. The true arm begins directly after the branch and
    /// ends where the false arm begins; the false arm ends at the function
    /// boundary.
    TopLevelTwoReturn {
        condition: TerminalScalarConditionalCondition,
        branch_offset: usize,
        branch_byte_count: usize,
        false_arm_offset: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalScalarConditionalCondition {
    Parameter,
    Expression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalScalarStackMutation {
    pub offset: usize,
    pub byte_count: usize,
    pub kind: TerminalScalarStackMutationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalScalarStackMutationKind {
    Allocate { byte_size: u32 },
    Release { byte_size: u32 },
    X86ReleasePreservingFlags { byte_size: u32 },
    X86Push,
    X86Pop,
}
