#![forbid(unsafe_code)]

//! Machine emission: the backend stages that turn one allocated
//! post-allocation machine plan into placed function text, and the
//! target-owned x86-64 byte recipes emitted outside that route.
//!
//! Native realization drives the route one retained stage at a time. Each
//! `stage_*` operation admits its source by replaying the previous stage's
//! custody, runs its production, and returns only after its own `validate_*`
//! replay accepts the result:
//!
//! 1. [`stage_fixed_frame_function_relative_realization`]
//!    ([`function_realization`]) replays the allocation, stages the frame,
//!    encodes and lays out the selected forms, and stages the whole-function
//!    exit contract.
//! 2. [`stage_optimized_function_fragment_emission`] ([`fragment_emission`])
//!    projects that realization into unplaced function fragments.
//! 3. [`stage_function_fragment_frame_application`] ([`frame_application`])
//!    inserts the frame's prologue and epilogue bytes into those fragments.
//! 4. [`stage_optimized_fixed_frame_text_section`] ([`text_placement`])
//!    places the framed fragments into one dense relocation-free text section.
//!
//! Each of the last three stages keeps admission and custody at its folder
//! root and its current-data mechanism, with an independent checker, one
//! level down: `fragment_emission::projection`, `frame_application::insertion`
//! and `text_placement::placement`.
//!
//! Realization joins three evidence owners, each with its own stage and
//! independent replay: [`frame_layout`] (frame geometry and callee-save
//! storage), [`frame_protocol`] (the frame's encoded prologue and epilogue
//! bytes) and [`exit_contract`] (whole-function exit evidence).
//!
//! Outside the route, [`startup_trampoline`], [`entry_exit_stub`] and
//! [`x86_fma`] emit target-owned x86-64 byte recipes with their own replay.
//! The object join over placed text belongs to object-file and image-emission.

// 1. Function-relative realization.
mod function_realization;

pub use function_realization::{
    stage_fixed_frame_function_relative_realization,
    validate_fixed_frame_function_relative_realization,
};

#[cfg(any(test, feature = "test-support"))]
pub use function_realization::{
    FixedFramePublicationCustodyFieldForTest, corrupt_fixed_frame_realization_custody_for_test,
    corrupt_fixed_frame_realization_exit_for_test,
    corrupt_fixed_frame_realization_manifest_for_test,
    replace_fixed_frame_realization_exit_for_test, swap_fixed_frame_realization_source_for_test,
};
pub use function_realization::{
    FunctionRelativeFrame, FunctionRelativeFrameDisposition,
    FunctionRelativeOptimizationRealizationError, FunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationManifestDecodeError,
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationRealizationStatistics, FunctionRelativeOptimizationUnavailableData,
    StagedFixedFrameFunctionRelativeRealization,
    StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
    ValidatedFunctionRelativeOptimizationRealizationManifest,
};
#[cfg(feature = "test-support")]
pub use function_realization::{
    corrupt_fixed_frame_realization_encoding_for_test,
    corrupt_fixed_frame_realization_layout_for_test,
};

// Evidence the realization joins: frame geometry, frame bytes, exit contract.
mod exit_contract;
pub mod frame_layout;
mod frame_protocol;

pub use frame_protocol::{
    stage_target_frame_protocol_encoding, validate_target_frame_protocol_encoding,
};

pub use frame_protocol::{
    FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding, TargetFrameProtocolEncodingError,
    TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy, TargetFrameProtocolEncodingReceipt,
    ValidatedTargetFrameProtocolEncoding, target_frame_protocol_encoding_identity,
};

pub use exit_contract::{
    stage_whole_function_exit_contract_after_x86_branch_relaxation,
    stage_whole_function_exit_contract_for_layout, stage_whole_function_exit_contract_with_frame,
    validate_whole_function_exit_contract_after_x86_branch_relaxation,
    validate_whole_function_exit_contract_for_layout,
    validate_whole_function_exit_contract_with_frame,
};

#[cfg(any(test, feature = "test-support"))]
pub use exit_contract::Rel8ExitBoundaryForTest;
pub use exit_contract::{
    ValidatedWholeFunctionExitContract, WholeFunctionEntryAssumption, WholeFunctionExitContract,
    WholeFunctionExitContractError, WholeFunctionExitContractIdentity, WholeFunctionExitEvidence,
    WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionFrameDisposition,
    WholeFunctionHardeningPolicy, WholeFunctionReturnEvidence, WholeFunctionReturnMechanism,
    WholeFunctionReturnValueEvidence,
};

// 2. Fragment emission.
mod fragment_emission;

pub use fragment_emission::{
    stage_optimized_function_fragment_emission, validate_optimized_function_fragment_emission,
};

#[cfg(any(test, feature = "test-support"))]
pub use fragment_emission::FunctionFragmentReplayInputs;
pub use fragment_emission::{
    FunctionFragmentEmissionError, FunctionFragmentEmissionManifest,
    FunctionFragmentEmissionManifestDecodeError, FunctionFragmentEmissionStage,
    FunctionFragmentEmissionStatistics, FunctionFragmentEmissionUnavailableData,
    StagedFunctionFragmentEmissionCustodyReceipt, StagedOptimizedFunctionFragmentEmission,
    StagedOptimizedFunctionFragmentEmissionSource, ValidatedFunctionFragmentEmissionManifest,
};

pub use fragment_emission::projection::{
    FunctionFragmentStatisticsOverflow, ResolvedFragmentEmissionError,
    emit_resolved_function_fragments, function_fragment_emission_statistics,
    validate_resolved_function_fragments,
};

// 3. Frame application.
mod frame_application;

pub use frame_application::{
    stage_function_fragment_frame_application, validate_function_fragment_frame_application,
};

pub use frame_application::{
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentFrameApplication,
    FunctionFragmentFrameApplicationError, FunctionFragmentFrameApplicationIdentity,
    FunctionFragmentFrameApplicationReceipt, StagedFunctionFragmentFrameApplication,
    function_fragment_frame_application_identity,
};

pub use frame_application::insertion::{
    FrameApplicationError, apply_frame_protocol_to_fragments, validate_frame_protocol_application,
};

// 4. Text placement.
mod text_placement;

pub use text_placement::{
    stage_optimized_fixed_frame_text_section, validate_optimized_fixed_frame_text_section,
};

#[cfg(any(test, feature = "test-support"))]
pub use text_placement::place_fragments_for_test;
pub use text_placement::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, RelocationFreeTextSectionPlacementError,
    StagedFixedFrameTextSectionCustodyReceipt, StagedOptimizedFixedFrameTextSection,
    ValidatedFunctionFragmentTextSectionManifest,
};

pub use text_placement::placement::{
    TextPlacementError, TextPlacementInput, place_fragment_text_section, text_section_statistics,
    validate_fragment_text_section,
};

// Target-owned x86-64 byte recipes outside the route.
mod entry_exit_stub;
mod startup_trampoline;
mod x86_fma;

pub use entry_exit_stub::{
    ValidatedX86_64DeriverStubEmission, ValidatedX86_64ResolvedDeriverStub,
    X86_64DeriverStubEmission, X86_64DeriverStubEmissionError, X86_64DeriverStubEmissionFootprint,
    X86_64DeriverStubMemberCall, X86_64DeriverStubRelocation, X86_64DeriverStubResolution,
    X86_64DeriverStubResolutionError, emit_x86_64_deriver_entry_exit_stub,
    resolve_x86_64_deriver_stub_member_call, validate_x86_64_deriver_entry_exit_stub,
    validate_x86_64_resolved_deriver_stub,
};
pub use startup_trampoline::{
    X86_64_STARTUP_TRAMPOLINE_BYTE_COUNT, X86_64ResolvedStartupTrampoline,
    X86_64StartupTrampolineFields, X86_64StartupTrampolineFootprint,
    X86_64StartupTrampolineResolution, X86_64StartupTrampolineResolutionError,
    X86_64StartupTrampolineTemplate, emit_x86_64_startup_trampoline,
    resolve_x86_64_startup_trampoline, validate_x86_64_startup_trampoline,
};
pub use x86_fma::{EmittedX86ScalarFmaFragment, emit_feature_required_x86_scalar_fma};
