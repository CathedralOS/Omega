//! The current emitted machine-code program and its concept owners.
//!
//! Functions own executable bytes and their records. Calls, storage, control
//! flow, ownership, and boundaries describe the facts replay must check.
//! Unplaced function fragments retain their own identity before image assembly.

pub mod boundary;
pub mod calls;
pub mod control_flow;
pub mod encoding;
pub mod fragments;
pub mod functions;
mod identity_encoding;
pub mod image_addresses;
pub mod instructions;
pub mod layout;
pub mod ownership;
pub mod provenance;
pub mod storage;

pub use boundary::{
    BoundaryByteSequenceArgumentRecord, BoundaryExecutionRecord, BoundaryResultRecord,
    BoundaryScalarResultRecord, BoundarySettlementRecord, BoundaryStructuralResultRecord,
    CompletionProviderCustodyBinding, PortEffectRecord, ProviderExecutionRecord,
    derive_completion_provider_custody, execution, ports, settlements,
};
pub use calls::{
    Aarch64ForeignCallFloatingControlRecord, CallbackAddressDestination, CallbackAddressEncoding,
    CallbackAddressMaterialization, ForeignCallRelocation, ForeignCallScalarArgumentRecord,
    ForeignCallScalarResultRecord, InstalledProviderUnitScalarCallRecord, InternalCallRelocation,
    InternalStructuralCallResult, InternalUnitCallArgumentRecord, InternalUnitCallRecord,
    InternalUnitCallSource, InternalUnitScalarArgumentSourceRecord,
    InternalUnitScalarCallArgumentRecord, InternalUnitScalarCallRecord,
    InternalUnitScalarCallResultRecord, InternalUnitStructuralArgumentSourceRecord,
    SelectedFormCallbackMaterialization, SelectedFormCallbackMaterializationEncoding,
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState, SelectedFormNormalizedForeignCallFixup,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
    StructuralCallScalarReturnEvidence, StructuralReturnRecord, arguments, callbacks, foreign,
    internal, results, unit_call_arguments,
};
pub use control_flow::{
    BooleanStructuralConditionEvidence, ScalarConditionalBranchEvidence,
    ScalarConditionalCondition, ScalarControlBlockEvidence, ScalarControlFlowEvidence,
    ScalarControlTerminatorEvidence, ScalarDirectConditionalBranchEvidence,
    ScalarDivisionBranchEvidence, ScalarJoinBranchEvidence,
};
pub use encoding::{
    DeferredControlEncodingReason, ResolvedPhysicalAddress, SelectedFormDecodedFootprint,
    SelectedFormDecodedFootprintHandle, SelectedFormEncodedByteSpan, SelectedFormEncodedRowStore,
    SelectedFormEncoding, SelectedFormEncodingCounts, SelectedFormEncodingIdentity,
    SelectedFormEncodingRow, SelectedFormEncodingState, SelectedFormMachineDisposition,
    SelectedFormMachineOptimizationCustody, SelectedFormMovnOptimizationCustody,
};
pub use fragments::{
    FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentBranchEvidence,
    FunctionFragmentCallbackMaterialization, FunctionFragmentCallbackMaterializationEncoding,
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentConditionalBranchPredicate,
    FunctionFragmentControlProvenance, FunctionFragmentEmissionManifest,
    FunctionFragmentEmissionManifestDecodeError, FunctionFragmentEmissionPlan,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData, FunctionFragmentInstructionSpan,
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupKind,
    FunctionFragmentInternalMachineFixupState, FunctionFragmentNormalizedForeignCallFixup,
    FunctionFragmentNormalizedForeignCallFixupKind,
    FunctionFragmentNormalizedForeignCallFixupState, FunctionFragmentSuccessorProvenance, fixups,
    identity, publication,
};
pub use functions::{
    CompilerPrivateMachineCodeFunction, MachineCodeFunction, WholeFunctionCrashEvidence,
    WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
    WholeFunctionExitEvidence, WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy,
    WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy, WholeFunctionProcessExitEvidence,
    WholeFunctionReturnEvidence, WholeFunctionReturnFragmentEvidence, WholeFunctionReturnMechanism,
    WholeFunctionReturnValueEvidence, exit_contract, whole_function_exit_contract_identity,
};
pub use image_addresses::{
    ByteLiteralReference, FunctionFragmentImageAddressFixup, ImageAddressFields,
    PlacedImageAddress, SelectedFormImageAddressFixup,
};
pub use instructions::{
    X86FloatingControlRecord, X86ForeignCallFloatingControlRecord, X86ScalarFmaFormat,
    X86ScalarFmaFragment, X86ScalarFmaOccurrenceRecord, x86_fma,
};
pub use layout::text_section::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, InternalMachineCallResolutionKind,
    InternalMachineCallResolutionState, NormalizedForeignCallResolutionKind,
    NormalizedForeignCallResolutionState, PlacedBlockSpan, PlacedCallbackAddressEncoding,
    PlacedCallbackAddressMaterialization, PlacedFunctionFragment, PlacedInstructionSpan,
    PlacedInternalMachineCallResolution, PlacedNormalizedForeignCallResolution,
    RelocationFreeTextSectionPlacement, TextSectionPlacementPolicy,
    TextSectionRelocationRequirements, relocation_free_text_section_identity,
};
pub use layout::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedJumpEvidence, ResolvedMachineLayout, ResolvedMachineProgram,
    ResolvedSelectedBlockLayout, ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy, X86BranchRelaxationIdentity,
    resolved_machine_layout_identity,
};
pub use ownership::{
    ScalarCleanupPreservationEvidence, ScalarControlAffineCleanupRecord, UnitAffineCleanupRecord,
    UnitContinuationRecord,
};
pub use provenance::{SemanticCodeAttribution, SemanticCodeSite, semantic_code_site};
pub(crate) use storage::Aarch64ReturnLinkEvidence;
pub use storage::{
    CalleeSaveFrameSlot, FrameProtocolByteSpan, FrameUnwindPlan, FrameUnwindRestore,
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentFrameApplication,
    FunctionFragmentFrameApplicationIdentity, FunctionNonAuthoritativeCalleeSaveStorage,
    FunctionTargetFrameLayout, FunctionTargetFrameProtocolEncoding, LocalStorageFrameSlot,
    NonAuthoritativeCalleeSaveSlot, NonAuthoritativeCalleeSaveSlotId,
    NonAuthoritativeCalleeSaveStorageIdentity, NonAuthoritativeCalleeSaveStoragePlan,
    NonAuthoritativeCalleeSaveStoragePolicy, OutgoingAbiFrameArea, ParameterFunctionAbiRecord,
    ReturnAddressFrameCustody, ScalarCallStackEvidence, ScalarStackEvidence, ScalarStackMutation,
    ScalarStackMutationKind, ScalarStructuralScalarFieldStoreRecord, StackAdjustmentPair,
    StackProbePlan, StructuralSourceLocation, TargetFrameLayoutIdentity, TargetFrameLayoutPlan,
    TargetFrameLayoutPolicy, TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy, UnitAffineScalarRecordEstablishmentRecord,
    UnitCallStackEvidence, UnitEntryRegisterSpillRecord, UnitIntegerConstantRecord,
    UnitParameterHomeRecord, UnitParameterRecord, UnitScalarHomeRecord,
    UnitScalarParameterLocationRecord, UnitStackEvidence, UnitStructuralScalarFieldStoreRecord,
    UnitWriteOnlyPrimitiveStoreRecord, UnitWriteOnlyPrimitiveStoreSourceRecord, frame_application,
    frame_identity, frame_layout, frame_protocol, function_fragment_frame_application_identity,
    parameters, scalars, stack, stores, target_frame_layout_identity,
    target_frame_protocol_encoding_identity,
};

use semantic_vocabulary::MachineId;
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineCodePlan {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    /// Identity text every `TerminalIdentityIndex` in this plan resolves
    /// against; the sealed module table travels with the plan verbatim.
    pub identities: terminal_psi::TerminalIdentityTable,
    pub functions: Vec<MachineCodeFunction>,
}

pub(crate) fn encode_units(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    units: &[register_model::RegisterUnitId],
) {
    hasher.u64(units.len() as u64);
    for unit in units {
        hasher.raw(&unit.0.to_le_bytes());
    }
}
