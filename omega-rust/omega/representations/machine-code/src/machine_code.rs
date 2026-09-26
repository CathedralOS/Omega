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
    CallbackAddressMaterialization, DynamicCallRecord, DynamicInstanceMaterializationRecord,
    DynamicParameterCallMechanismRecord, DynamicParameterCallRecord, DynamicTableAddressEncoding,
    DynamicTableAddressMaterialization, DynamicTraitDescriptorAbiRecord, ForeignCallRelocation,
    ForeignCallScalarArgumentRecord, ForeignCallScalarResultRecord,
    ForwardedDynamicDescriptorAdapterIdentity, ForwardedDynamicDescriptorAdapterRecord,
    ForwardedDynamicDescriptorArgumentRecord, ForwardedDynamicDescriptorCallRecord,
    ForwardedDynamicParameterCallRecord, ForwardedDynamicParameterCallStackEvidence,
    InstalledProviderUnitScalarCallRecord, InternalCallRelocation, InternalStructuralCallResult,
    InternalStructuralResultHomeRecord, InternalUnitCallArgumentRecord, InternalUnitCallRecord,
    InternalUnitCallSource, InternalUnitScalarArgumentSourceRecord,
    InternalUnitScalarCallArgumentRecord, InternalUnitScalarCallRecord,
    InternalUnitScalarCallResultRecord, InternalUnitStructuralArgumentSourceRecord,
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState, SelectedFormNormalizedForeignCallFixup,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
    StoredDynamicCallRecord, StoredDynamicDescriptorMaterializationRecord,
    StructuralCallScalarReturnEvidence, StructuralReturnRecord, arguments, callbacks, dynamic,
    foreign, internal, results,
};
pub use control_flow::{
    BooleanStructuralConditionEvidence, BooleanStructuralFieldRead,
    ScalarConditionalBranchEvidence, ScalarConditionalCondition, ScalarControlBlockEvidence,
    ScalarControlFlowEvidence, ScalarControlTerminatorEvidence,
    ScalarDirectConditionalBranchEvidence, ScalarDivisionBranchEvidence, ScalarJoinBranchEvidence,
};
pub use encoding::{
    DeferredControlEncodingReason, ResolvedPhysicalAddress, SelectedFormDecodedFootprint,
    SelectedFormEncoding, SelectedFormEncodingCounts, SelectedFormEncodingIdentity,
    SelectedFormEncodingRow, SelectedFormEncodingState, SelectedFormMachineDisposition,
    SelectedFormMachineOptimizationCustody, SelectedFormMovnOptimizationCustody,
};
pub use fragments::{
    FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentBranchEvidence,
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentConditionalBranchPredicate,
    FunctionFragmentControlProvenance, FunctionFragmentEmissionManifest,
    FunctionFragmentEmissionManifestDecodeError, FunctionFragmentEmissionPlan,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData, FunctionFragmentInstructionSpan,
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupKind,
    FunctionFragmentInternalMachineFixupState, FunctionFragmentJumpEvidence,
    FunctionFragmentNormalizedForeignCallFixup, FunctionFragmentNormalizedForeignCallFixupKind,
    FunctionFragmentNormalizedForeignCallFixupState, FunctionFragmentSuccessorProvenance, fixups,
    function_fragment_emission_identity, identity, publication,
};
pub use functions::{
    CompilerPrivateMachineCodeFunction, MachineCodeFunction, MachineCodePlanWithPrivateFunctions,
    WholeFunctionCrashEvidence, WholeFunctionEntryAssumption, WholeFunctionExitContract,
    WholeFunctionExitContractIdentity, WholeFunctionExitEvidence, WholeFunctionExitLayoutCustody,
    WholeFunctionExitPolicy, WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy,
    WholeFunctionProcessExitEvidence, WholeFunctionReturnEvidence,
    WholeFunctionReturnFragmentEvidence, WholeFunctionReturnMechanism,
    WholeFunctionReturnValueEvidence, exit_contract, whole_function_exit_contract_identity,
};
pub use instructions::{
    X86FloatingControlRecord, X86ForeignCallFloatingControlRecord, X86ScalarFmaFormat,
    X86ScalarFmaFragment, X86ScalarFmaOccurrenceRecord, X86ScalarFmaOperandRecord, x86_fma,
    x86_scalar_fma_fragment_identity,
};
pub use layout::text_section::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, InternalMachineCallResolutionKind,
    InternalMachineCallResolutionState, NormalizedForeignCallResolutionKind,
    NormalizedForeignCallResolutionState, PlacedBlockSpan, PlacedFunctionFragment,
    PlacedInstructionSpan, PlacedInternalMachineCallResolution,
    PlacedNormalizedForeignCallResolution, RelocationFreeTextSectionPlacement,
    TextSectionPlacementPolicy, TextSectionRelocationRequirements,
    relocation_free_text_section_identity,
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
pub use provenance::{SemanticCodeAttribution, SemanticCodeSite};
pub use storage::{
    Aarch64ReturnLinkEvidence, CalleeSaveFrameSlot, FrameProtocolByteSpan, FrameUnwindPlan,
    FrameUnwindRestore, FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol,
    FunctionFragmentFrameApplication, FunctionFragmentFrameApplicationIdentity,
    FunctionNonAuthoritativeCalleeSaveStorage, FunctionSpillFrameRequirements,
    FunctionTargetFrameLayout, FunctionTargetFrameProtocolEncoding, LocalStorageFrameSlot,
    NonAuthoritativeCalleeSaveSlot, NonAuthoritativeCalleeSaveSlotId,
    NonAuthoritativeCalleeSaveStorageIdentity, NonAuthoritativeCalleeSaveStoragePlan,
    NonAuthoritativeCalleeSaveStoragePolicy, NonAuthoritativeSpillFrameRequirementIdentity,
    NonAuthoritativeSpillFrameRequirementPlan, NonAuthoritativeSpillFrameRequirementPolicy,
    OutgoingAbiFrameArea, ParameterFunctionAbiRecord, ReturnAddressFrameCustody,
    ScalarCallStackEvidence, ScalarStackEvidence, ScalarStackMutation, ScalarStackMutationKind,
    ScalarStructuralScalarFieldStoreRecord, StackAdjustmentPair, StackProbePlan,
    StructuralSourceLocation, TargetFrameLayoutIdentity, TargetFrameLayoutPlan,
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
    pub functions: Vec<MachineCodeFunction>,
}
