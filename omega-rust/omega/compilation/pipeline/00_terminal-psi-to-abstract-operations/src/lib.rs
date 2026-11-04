#![forbid(unsafe_code)]

//! Terminal Psi to abstract operations: the first Omega program stage.
//!
//! Native production enters `admit_native_realization` (`native_admission`):
//! canonical input preparation, deployment execution scope, provider settlement
//! and installation, and authority review produce one input for Omega 01.
//! `native_execution_scope` selects the actual call/cleanup/provider closure
//! without erasing canonical storage. Private callback artifacts use the same
//! selection before their physical stages; portable consumers stay complete.
//! Portable consumers use `lower_artifact` (`artifact_admission`). It replays
//! and verifies a Terminal artifact's semantic and proof sections, then lowers
//! each machine (`lowering`) into source-independent abstract operations.
//! Two other entrances serve the same verified input:
//! `build_verified_psi_optimization_unit` (`optimization`) builds the Psi
//! optimization unit the optimizer rewrites, and
//! `provider_admission::admit_checked_provider_installation` joins provider
//! installation custody to the admitted artifact's retained module.
//!
//! Artifact admission is the one place in Omega that decodes a Terminal
//! module from bytes; every later consumer receives the decoded module
//! (`terminal_decode_once.rs` holds that by scanning the sources).
//!
//! `entry_settlement` and `callback_settlement` admit native entry sidecars
//! against already-produced Terminal artifacts. These checks do not create
//! another program stage or grant artifact-publication authority.
//! `comparison_settlement` consumes Terminal operations and settled boundary
//! executions. It does not recover source uses or repeat provider selection.
//! `retained_terminal::proposal` owns their producing sequence, joining already
//! compiled entry/callback artifacts to selected native evidence once. The
//! retained proposal never leaves the process, so no later stage replays these
//! joins; the compiler supplies products and selections rather than
//! reconstructing them.

mod artifact_admission;
mod lowering;
mod native_admission;
mod native_execution_scope;
mod optimization;
// Provider admission and its sealed evidence live in their own package; this
// stage runs the admission and keeps reading it under the same module name.
use ::provider_admission as providers;
pub use ::provider_admission::provider_admission;
mod retained_terminal;
mod settlement;
#[cfg(test)]
mod terminal_decode_once;

pub use native_admission::{
    AdmittedNativeRealization, NativeAdmissionRequest, admit_native_realization,
    admit_prepared_native_realization, artifact_lowering_error,
};
pub use providers::provider_admission::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
    NativeCallbackArgumentProposal, SettledCompilerBuiltin,
};
pub use retained_terminal::{
    AdmittedNativeComponentCall, ComponentExportInput, ComponentNativeInputs,
    PreparedNativeExecution, PreparedNativeWork, PreparedRetainedNativeProviders,
    RetainedComponentNativeInputs, RetainedNativeProviderInputs, RetainedTerminalArtifact,
    SelectedFamilyRealizationSource, TerminalCallbackOccurrenceProposal,
    TerminalCallbackThunkArtifact, TerminalCompilerBuiltinProposal,
    TerminalIeeeFloatComparisonOccurrenceProposal, TerminalIntegerComparisonOccurrenceProposal,
    TerminalNativeBoundaryApplications, TerminalNativeOccurrences, TerminalNativeProposalRequest,
    TerminalNativeProviders, TerminalNativeRealizationProposal, TerminalTargetProviderProposal,
    project_terminal_native_realization_proposal, recover_native_derivation_inputs,
    retain_native_derivation_inputs,
};

pub(crate) use artifact_admission::ArtifactLoweringError;
pub use artifact_admission::{
    ArtifactSections, NativeExecutionSelection, NativeScalarApplications,
    TerminalAccessViewEstablishment, VerifiedNativeArtifactInput, lower_artifact,
};
pub use lowering::LoweringError;
pub use optimization::{
    VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnit,
    VerifiedPsiOptimizationUnitBuildError, build_verified_psi_optimization_unit,
};
pub use providers::provider_installation::SelectedProviderAdapter;
pub use providers::provider_installation::{
    AdmittedProviderInstallation, ProviderInstallationError,
};
pub use settlement::boundary_application_settlement::BoundaryApplicationSettlement;
pub use settlement::callback_settlement::settle_native_callback_placements;
pub(crate) use settlement::callback_settlement::{
    CallbackRegistrarOccurrence, settle_callback_registrar_imports,
};
pub use settlement::entry_settlement::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError, SelectedProgramEntryCall,
    ValidatedNativeProgramEntrySettlement, validate_native_program_entry_settlement,
};
