//! Internal call sites and their semantic, ABI, and byte evidence.

use crate::{
    InternalUnitCallArgumentRecord, InternalUnitScalarCallArgumentRecord,
    InternalUnitScalarCallResultRecord, ScalarCallStackEvidence, UnitCallStackEvidence,
};
use abstract_operations::AbstractResult;
use calling_conventions::{CallPlan, ValuePlacement};
use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, EdgeId, MachineId, OperationId, ScalarType, ValueId,
};
use target_operations::CallSiteOwner;
use terminal_psi::{
    ClaimTransfer, CompletionReceipt, ProviderCandidateConformance, StructuralArgument,
    StructuralResultDeclaration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralCallScalarReturnEvidence {
    pub psi_edge: EdgeId,
    pub psi_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub callee: MachineId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternalCallRelocation {
    /// Exact semantic owner of the encoded call. Cleanup-edge calls retain an
    /// [`EdgeId`] here instead of borrowing or inventing an operation identity.
    pub owner: CallSiteOwner,
    pub target: MachineId,
    /// Exact caller-owned stack live immediately after this aggregate-frame
    /// call enters its callee. Absent for function forms whose full stack
    /// accounting has not yet migrated.
    pub unit_stack: Option<UnitCallStackEvidence>,
    /// Exact scalar-expression call-site stack facts. The object boundary
    /// replays the surrounding ordered mutations and derives caller-live
    /// bytes; this evidence names only the encoded outbound area and, on
    /// AArch64, the explicit link-register save/restore.
    pub scalar_stack: Option<ScalarCallStackEvidence>,
    /// Byte offset within this function at which the relocation field begins.
    /// On x86-64 this points at the four-byte displacement following `CALL`;
    /// on AArch64 it points at the `BL` instruction word.
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalUnitCallRecord {
    pub source: InternalUnitCallSource,
    /// Must match the owner of the corresponding relocation or independently
    /// resolved internal-machine fixup exactly.
    pub owner: CallSiteOwner,
    pub target: MachineId,
    /// `None` for a value-less structural call; otherwise the exact scalar
    /// result returned through the ordinary target ABI.
    pub result: Option<ScalarType>,
    /// Exact Terminal scalar value produced by this call when later semantic
    /// custody needs to distinguish production from return. The ABI result
    /// above remains the independently replayed physical type.
    pub semantic_result: Option<AbstractResult>,
    /// Exact structural result custody for the bounded direct-return call
    /// family. Mutually exclusive with `result`; absent on Unit/scalar calls.
    pub structural_result: Option<InternalStructuralCallResult>,
    /// Exact scalar arguments in the scalar prefix of the shared call plan.
    /// Aggregate-only calls retain the canonical empty roster.
    pub scalar_arguments: Vec<InternalUnitScalarCallArgumentRecord>,
    pub arguments: Vec<InternalUnitCallArgumentRecord>,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalStructuralCallResult {
    pub operation_result: terminal_psi::StructuralOperationResult,
    pub function_result: StructuralResultDeclaration,
    pub returned_claim_transfers: Vec<terminal_psi::StructuralResultClaimTransfer>,
    pub returned_claims: Vec<ClaimId>,
    pub caller_result_placement: ValuePlacement,
    pub callee_result_placement: ValuePlacement,
}

/// Real emitted fixed-width integer call in an attached Unit body.
///
/// The relocation validates the call instruction and target; this record owns
/// the argument and result-store bytes around it. Object construction must
/// independently reconstruct every byte before admitting the artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalUnitScalarCallRecord {
    pub owner: CallSiteOwner,
    pub target: MachineId,
    pub call_plan: CallPlan,
    pub result: InternalUnitScalarCallResultRecord,
    pub arguments: Vec<InternalUnitScalarCallArgumentRecord>,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// One emitted Unit-returning fixed-i32 call through an exact selected
/// provider. The internal relocation owns the mutable call field; this row
/// owns the complete semantic selection and scalar argument interval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProviderUnitScalarCallRecord {
    pub owner: CallSiteOwner,
    pub boundary: BoundaryMachineId,
    pub provider: ProviderCandidateConformance,
    pub call_plan: CallPlan,
    pub arguments: Vec<InternalUnitScalarCallArgumentRecord>,
    pub source_arguments: Vec<StructuralArgument>,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub completion_claim_sources: Vec<target_operations::CompletionClaimSource>,
    pub completion_receipts: Vec<CompletionReceipt>,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// Semantic origin is independent of the resolved call encoding.
/// Semantic origin retained independently of equal internal-call bytes.
/// Installed provider declarations and claim receipts are custody, not fresh
/// provider admission authority. Publication replays them against the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternalUnitCallSource {
    Authored,
    InstalledProvider {
        boundary: BoundaryMachineId,
        provider: Box<ProviderCandidateConformance>,
        completion_claim_sources: Vec<abstract_operations::CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
}
