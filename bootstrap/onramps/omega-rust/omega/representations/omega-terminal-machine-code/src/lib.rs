#![forbid(unsafe_code)]

//! Owned target machine code emitted from the clean terminal-Psi realization
//! lane.

use omega_calling_conventions::{ValuePlacement, ValueShape};
use omega_target::NativeTarget;
use omega_terminal_installation_evidence::NativeFuelTargetPlanProjection;
use omega_terminal_target_operations::{
    TerminalBoundaryRealization, TerminalBoundaryScalarArgument, TerminalCallSiteOwner,
    TerminalCompletionClaimSource, TerminalProviderExecutionBinding, TerminalPsiProvenance,
};
use psi_core::{
    BoundaryMachineId, ClaimId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralFieldId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    ClaimTransfer, CompletionReceipt, StructuralArgument, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, TerminalAffineCleanupAction, TerminalPsiIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachineCodePlan {
    pub terminal_psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<TerminalMachineCodeFunction>,
}

/// Metered bytes derived from one immutable, already emitted semantic plan.
/// The source plan remains intact so object validation can replay every
/// semantic interval independently; charge records provide the exact mapping
/// into the derived byte stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeFuelInstrumentedPlan {
    pub source: TerminalMachineCodePlan,
    pub target_policy: NativeFuelTargetPlanProjection,
    pub functions: Vec<TerminalNativeFuelInstrumentedFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeFuelInstrumentedFunction {
    pub machine: MachineId,
    pub bytes: Vec<u8>,
    /// End of semantic code and start of appended cold dispatch thunks.
    pub semantic_end_offset: usize,
    pub charges: Vec<TerminalNativeFuelChargeRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalNativeFuelChargeRecord {
    /// Original semantic attribution retained by the immutable source plan.
    pub attribution: TerminalNativeFuelAttribution,
    pub charge_code_offset: usize,
    pub charge_byte_count: usize,
    /// Position of the unchanged semantic interval in the metered bytes.
    pub semantic_code_offset: usize,
    pub cold_dispatch_code_offset: usize,
    pub cold_dispatch_byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachineCodeFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub bytes: Vec<u8>,
    /// Target-emitter-owned stack facts for the aggregate-frame body closure:
    /// Unit bodies and the bounded direct structural-call/scalar-return
    /// carrier. Other terminal function forms remain deliberately unreported
    /// until their complete temporary-stack accounting is retained.
    pub unit_stack: Option<TerminalUnitStackEvidence>,
    /// Complete ordered incoming structural-parameter homes for an aggregate-
    /// frame body.
    /// Object validation binds projected-call custody to this independently
    /// retained caller frame plan instead of trusting per-call offsets.
    pub unit_parameter_homes: Vec<TerminalUnitParameterHomeRecord>,
    /// Independent ordered semantic signature for an aggregate-frame body. Object
    /// validation binds each mutable ABI home back to this declaration row.
    pub unit_parameters: Vec<TerminalUnitParameterRecord>,
    /// Exact ordered stack mutations and admitted control-flow shape for a
    /// scalar function. Object construction replays the target instructions
    /// and derives the numeric peak; unsupported scalar forms remain `None`.
    pub scalar_stack: Option<TerminalScalarStackEvidence>,
    /// Typed internal-call relocation fields, ordered by `offset`. Each row
    /// points at the mutable immediate bits of one architecture-native call;
    /// object construction validates the surrounding opcode before accepting
    /// the relocation.
    pub internal_calls: Vec<TerminalInternalCallRelocation>,
    /// Complete ordered semantic and ABI custody for in-module Unit calls.
    pub internal_unit_calls: Vec<TerminalInternalUnitCallRecord>,
    /// Exact zero-code affine-local establishment and Unit-return cleanup
    /// custody for the bounded one-state Unit slice.
    pub unit_affine_cleanup: Option<TerminalUnitAffineCleanupRecord>,
    /// Structural custody consumed by a scalar return after its result has
    /// been materialized. The record deliberately reuses the exact cleanup
    /// vocabulary while remaining distinct from a Unit body.
    pub scalar_affine_cleanup: Option<TerminalUnitAffineCleanupRecord>,
    /// Canonical true-before-false DFS leaves for the exact bounded
    /// two-decision/three-return Boolean-control carrier. Each row binds one
    /// real terminal-Psi return edge and physical cleanup suffix to the
    /// independently replayable result/link preservation for that suffix.
    /// Branch-free scalar cleanup continues to use `scalar_affine_cleanup`.
    pub scalar_control_affine_cleanups: Vec<TerminalScalarControlAffineCleanupRecord>,
    pub scalar_structural_parameters: Vec<TerminalUnitParameterRecord>,
    pub scalar_structural_parameter_homes: Vec<TerminalUnitParameterHomeRecord>,
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
pub struct TerminalUnitAffineCleanupRecord {
    pub psi_edge: EdgeId,
    /// Canonical verifier-owned type closure retained so object and installed
    /// artifact validation can reconstruct the exact residual partition.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub locals: Vec<(
        OperationId,
        StructuralPlaceDeclaration,
        StructuralTypeDeclaration,
    )>,
    /// Exact semantic execution order for every cleanup committed by this edge.
    pub actions: Vec<TerminalAffineCleanupAction>,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarControlAffineCleanupRecord {
    pub cleanup: TerminalUnitAffineCleanupRecord,
    pub preservation: TerminalScalarCleanupPreservationEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalUnitParameterHomeRecord {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: psi_terminal::StructuralMultiplicity,
    pub shape: ValueShape,
    pub source: ValuePlacement,
    pub byte_offset: u32,
    pub indirect: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalUnitParameterRecord {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: psi_terminal::StructuralMultiplicity,
    pub shape: ValueShape,
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
pub struct TerminalBoundaryResultRecord {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub placement: ValuePlacement,
    /// Exact terminal edge that returns this boundary-produced value.
    pub return_edge: EdgeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundarySettlementRecord {
    pub psi_operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub provider_execution: TerminalProviderExecutionRecord,
    pub realization: TerminalBoundaryRealization,
    /// Ordered scalar inputs consumed by executable boundary code. Metadata-
    /// only settlements retain an empty list; native realizations retain the
    /// exact terminal value, type, immediate, and ABI destination.
    pub scalar_arguments: Vec<TerminalBoundaryScalarArgument>,
    /// Exact typed Psi custody arguments, including structural projections.
    /// These are provider-settlement evidence and do not describe an internal
    /// Unit-call ABI.
    pub arguments: Vec<StructuralArgument>,
    /// Exact native byte-sequence arguments and their independently replayable
    /// semantic and physical custody.
    pub byte_sequence_arguments: Vec<TerminalBoundaryByteSequenceArgumentRecord>,
    /// Complete canonical caller claim-source catalog needed to independently
    /// reconstruct the exact successful-completion receipt set.
    pub completion_claim_sources: Vec<TerminalCompletionClaimSource>,
    pub completion_receipts: Vec<CompletionReceipt>,
    /// Exact structural join between each successful-completion receipt, its
    /// retained caller source, and the admitted provider execution that owns
    /// this settlement. This is custody evidence only: it does not authorize
    /// content introduction, prove backing, or derive residual geometry.
    pub completion_provider_custody: Vec<TerminalCompletionProviderCustodyBinding>,
    /// Exact terminal value identity, scalar type, native placement, and
    /// returning edge consumed by a result-bearing realization. Metadata-only
    /// settlements retain `None` and cannot manufacture a result after
    /// lowering.
    pub native_result: Option<TerminalBoundaryResultRecord>,
    /// Position in the verified Unit operation sequence. This remains the
    /// canonical tie-break when multiple metadata rows share a code offset.
    pub operation_ordinal: usize,
    /// Byte offset immediately after all preceding executable operations.
    pub code_offset: usize,
    /// Zero for metadata-only settlements; otherwise the exact provider
    /// instruction interval that produces the boundary result.
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundaryByteSequenceArgumentRecord {
    pub argument: StructuralArgument,
    pub literal_operation: OperationId,
    pub structural_type: StructuralTypeDeclaration,
    pub bytes: Vec<u8>,
    /// Function-relative executable interval that consumes this payload.
    pub code_offset: usize,
    pub code_byte_count: usize,
    /// Function-relative exact literal-plus-newline interval.
    pub data_offset: usize,
    pub data_byte_count: usize,
}

/// Non-authoritative artifact custody for one exact completion receipt.
///
/// The row deliberately repeats the exact source and provider-execution
/// records instead of replacing either with a producer-authored aggregate or
/// authorization fingerprint. Object and installation validation rederive the
/// complete ordered catalog from the enclosing settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCompletionProviderCustodyBinding {
    pub source: TerminalCompletionClaimSource,
    pub receipt: CompletionReceipt,
    pub provider_execution: TerminalProviderExecutionRecord,
}

pub fn derive_completion_provider_custody(
    provider_execution: TerminalProviderExecutionRecord,
    sources: &[TerminalCompletionClaimSource],
    receipts: &[CompletionReceipt],
) -> Option<Vec<TerminalCompletionProviderCustodyBinding>> {
    receipts
        .iter()
        .map(|receipt| {
            let source = sources
                .iter()
                .find(|source| source.claim() == receipt.claim)?;
            Some(TerminalCompletionProviderCustodyBinding {
                source: source.clone(),
                receipt: *receipt,
                provider_execution,
            })
        })
        .collect()
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
    /// Exact semantic owner of the encoded call. Cleanup-edge calls retain an
    /// [`EdgeId`] here instead of borrowing or inventing an operation identity.
    pub owner: TerminalCallSiteOwner,
    pub target: MachineId,
    /// Exact caller-owned stack live immediately after this aggregate-frame
    /// call enters its callee. Absent for function forms whose full stack
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInternalUnitCallRecord {
    /// Must match the owner of the corresponding relocation exactly.
    pub owner: TerminalCallSiteOwner,
    pub target: MachineId,
    /// `None` for a value-less structural call; otherwise the exact scalar
    /// result returned through the ordinary target ABI.
    pub result: Option<ScalarType>,
    pub arguments: Vec<TerminalInternalUnitCallArgumentRecord>,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInternalUnitCallArgumentRecord {
    pub place: PlaceId,
    pub access: psi_terminal::StructuralAccess,
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    pub source_byte_offset: u32,
    pub source_home_byte_offset: u32,
    pub call_stack_bytes: u32,
    pub fixed_array_length: Option<u64>,
    pub element_stride: Option<u32>,
    pub source: ValuePlacement,
    pub destination: ValuePlacement,
    pub code_offset: usize,
    pub byte_count: usize,
    /// Immutable target bytes that realize this exact source-to-destination copy.
    pub bytes: Vec<u8>,
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
    /// Exact ABI-result preservation around an appended scalar-return cleanup
    /// suffix. Ordinary scalar functions have no such suffix and retain
    /// `None`; object construction validates every named access independently
    /// from the generic mutation trace.
    pub cleanup_preservation: Option<TerminalScalarCleanupPreservationEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalScalarCleanupPreservationEvidence {
    pub frame: TerminalStackAdjustmentPair,
    pub result_byte_offset: u32,
    pub result_store_offset: usize,
    pub result_load_offset: usize,
    /// AArch64 additionally preserves the caller's link register in the same
    /// lifetime frame. X86-64 uses its implicit stack link and retains `None`.
    pub aarch64_return_link: Option<TerminalAarch64ReturnLinkEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalScalarControlFlowEvidence {
    Linear,
    /// A direct scalar return containing one or more compiler-generated x86-64
    /// signed division/remainder diamonds. Each branch selects the `-1`
    /// overflow handling arm or the ordinary DIV/IDIV arm, and both paths
    /// reconverge before expression evaluation continues. The object boundary
    /// validates both branch targets and replays the two stack paths
    /// independently.
    LinearWithDivisionBranches {
        branches: Vec<TerminalScalarDivisionBranchEvidence>,
    },
    /// One acyclic Boolean decision tree whose branches are retained in
    /// increasing physical code order. Its terminal bitmap is in physical
    /// true-before-false DFS order and therefore contains exactly one more
    /// entry than `decisions`. Ordered x86 division diamonds are partitioned
    /// across decision prefixes and returning leaves during object replay.
    ConditionalTree {
        decisions: Vec<TerminalScalarConditionalBranchEvidence>,
        crash_leaves: Vec<bool>,
        branches: Vec<TerminalScalarDivisionBranchEvidence>,
    },
    /// One native Boolean decision tree whose ordered value leaves reconverge
    /// at an exact shared return/cleanup tail. Every non-final leaf ends in one
    /// retained unconditional branch targeting `merge_offset`; the final leaf
    /// falls through to that same offset.
    BooleanSharedConvergence {
        decisions: Vec<TerminalScalarConditionalBranchEvidence>,
        joins: Vec<TerminalScalarJoinBranchEvidence>,
        /// Exact emitted condition regions containing structural-field reads.
        /// Object replay checks these bytes independently from the generic
        /// scalar instruction walk before accepting the shared tail.
        structural_conditions: Vec<TerminalBooleanStructuralConditionEvidence>,
        merge_offset: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBooleanStructuralConditionEvidence {
    pub reads: Vec<TerminalBooleanStructuralFieldRead>,
    pub code_offset: usize,
    pub byte_count: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalBooleanStructuralFieldRead {
    pub psi_operation: OperationId,
    pub source: PlaceId,
    pub field: StructuralFieldId,
    pub field_byte_offset: u32,
    /// Exact native interval which loads this field and normalizes it to a
    /// Boolean result. Object replay reconstructs these bytes independently
    /// from the retained structural home and canonical layout.
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalScalarJoinBranchEvidence {
    pub join_offset: usize,
    pub join_byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalScalarDivisionBranchEvidence {
    pub branch_offset: usize,
    pub branch_byte_count: usize,
    pub ordinary_arm_offset: usize,
    pub join_offset: usize,
    pub join_byte_count: usize,
    pub merge_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalScalarConditionalBranchEvidence {
    pub condition: TerminalScalarConditionalCondition,
    pub branch_offset: usize,
    pub branch_byte_count: usize,
    pub false_arm_offset: usize,
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
