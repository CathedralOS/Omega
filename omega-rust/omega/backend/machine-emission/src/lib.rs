#![forbid(unsafe_code)]

//! Common physical-graph realization, frame application, and fragment emission.
//!
//! Function realization retains checked allocation, encoding, frame, and layout inputs.
//! Fragment emission, frame application, and text placement independently replay them.
//!
//! Route, in the order native realization drives it:
//!
//! - [`function_realization`] stages the function-relative realization over the
//!   retained allocation, encoding, [`frame_layout`], and [`frame_protocol`]
//!   inputs; [`exit_contract`] stages the whole-function exit contract inside it.
//! - [`fragments`] emits resolved machine data as unplaced function fragments.
//! - [`fragment_emission`] stages and independently validates that emission.
//! - [`frame_application`] applies the target frame protocol bytes to the fragments.
//! - [`text_placement`] places the framed fragments into one dense text section.
//!
//! The object join over the placed text belongs to image-emission. Every public
//! name is re-exported below from the module that owns it.

mod function_realization;
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
    stage_fixed_frame_function_relative_realization,
    validate_fixed_frame_function_relative_realization,
};
#[cfg(any(test, feature = "test-support"))]
#[cfg(feature = "test-support")]
pub use function_realization::{
    corrupt_fixed_frame_realization_encoding_for_test,
    corrupt_fixed_frame_realization_layout_for_test,
};
mod fragment_emission;
#[cfg(any(test, feature = "test-support"))]
pub use fragment_emission::FunctionFragmentReplayInputs;
pub use fragment_emission::{
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentEmissionError,
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionManifestDecodeError,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData, FunctionFragmentFrameApplication,
    FunctionFragmentFrameApplicationError, FunctionFragmentFrameApplicationIdentity,
    FunctionFragmentFrameApplicationReceipt, StagedFunctionFragmentEmissionCustodyReceipt,
    StagedFunctionFragmentFrameApplication, StagedOptimizedFunctionFragmentEmission,
    StagedOptimizedFunctionFragmentEmissionSource, ValidatedFunctionFragmentEmissionManifest,
    function_fragment_frame_application_identity, stage_function_fragment_frame_application,
    stage_optimized_function_fragment_emission, validate_function_fragment_frame_application,
    validate_optimized_function_fragment_emission,
};
mod text_placement;
#[cfg(any(test, feature = "test-support"))]
pub use text_placement::custody::place_fragments_for_test;
pub use text_placement::custody::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, RelocationFreeTextSectionPlacementError,
    StagedFixedFrameTextSectionCustodyReceipt, StagedOptimizedFixedFrameTextSection,
    ValidatedFunctionFragmentTextSectionManifest, stage_optimized_fixed_frame_text_section,
    validate_optimized_fixed_frame_text_section,
};
pub use text_placement::{
    TextPlacementError, TextPlacementInput, place_fragment_text_section, text_section_statistics,
    validate_fragment_text_section,
};

mod exit_contract;
#[cfg(any(test, feature = "test-support"))]
pub use exit_contract::Rel8ExitBoundaryForTest;
pub use exit_contract::{
    ValidatedWholeFunctionExitContract, WholeFunctionEntryAssumption, WholeFunctionExitContract,
    WholeFunctionExitContractError, WholeFunctionExitContractIdentity, WholeFunctionExitEvidence,
    WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionFrameDisposition,
    WholeFunctionHardeningPolicy, WholeFunctionReturnEvidence, WholeFunctionReturnMechanism,
    WholeFunctionReturnValueEvidence, stage_whole_function_exit_contract,
    stage_whole_function_exit_contract_after_x86_branch_relaxation,
    stage_whole_function_exit_contract_for_layout, stage_whole_function_exit_contract_with_frame,
    validate_whole_function_exit_contract,
    validate_whole_function_exit_contract_after_x86_branch_relaxation,
    validate_whole_function_exit_contract_for_layout,
    validate_whole_function_exit_contract_with_frame,
};
mod frame_application;
pub mod frame_layout;
mod frame_protocol;
pub use frame_application::{
    FrameApplicationError, apply_frame_protocol_to_fragments, validate_frame_protocol_application,
};
pub use frame_protocol::{
    FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding, TargetFrameProtocolEncodingError,
    TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy, TargetFrameProtocolEncodingReceipt,
    ValidatedTargetFrameProtocolEncoding, stage_target_frame_protocol_encoding,
    target_frame_protocol_encoding_identity, validate_target_frame_protocol_encoding,
};
mod fragments;
pub use fragments::{
    FunctionFragmentStatisticsOverflow, ResolvedFragmentEmissionError,
    emit_resolved_function_fragments, function_fragment_emission_statistics,
    validate_resolved_function_fragments,
};

mod x86_fma;
pub use x86_fma::{EmittedX86ScalarFmaFragment, emit_feature_required_x86_scalar_fma};

mod entry_exit_stub;
pub use entry_exit_stub::{
    ValidatedX86_64DeriverStubEmission, ValidatedX86_64ResolvedDeriverStub,
    X86_64DeriverStubEmission, X86_64DeriverStubEmissionError, X86_64DeriverStubEmissionFootprint,
    X86_64DeriverStubMemberCall, X86_64DeriverStubRelocation, X86_64DeriverStubResolution,
    X86_64DeriverStubResolutionError, emit_x86_64_deriver_entry_exit_stub,
    resolve_x86_64_deriver_stub_member_call, validate_x86_64_deriver_entry_exit_stub,
    validate_x86_64_resolved_deriver_stub,
};
