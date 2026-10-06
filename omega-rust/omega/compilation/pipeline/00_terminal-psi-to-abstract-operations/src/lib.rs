#![forbid(unsafe_code)]

//! Terminal Psi to abstract operations: the first Omega program stage.
//!
//! Native production enters `admit_native_realization` (`native_admission`):
//! canonical input preparation, provider settlement and installation, and
//! authority review produce one complete admitted input for stage 01.
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
//! `comparison_settlement` joins emitted comparison occurrences to authored
//! applications and this target's selected providers. It produces source-free
//! comparison records, not a substitute for canonical artifact admission.
//! `retained_terminal::proposal` owns their producing sequence, joining already
//! compiled entry/callback artifacts to selected native evidence once. The
//! retained proposal never leaves the process, so no later stage replays these
//! joins; the compiler supplies products and selections rather than
//! reconstructing them.

mod artifact_admission;
mod boundary_application_settlement;
mod callback_settlement;
mod comparison_settlement;
mod entry_settlement;
mod lowering;
mod native_admission;
mod optimization;
pub mod provider_admission;
mod provider_installation;
mod retained_terminal;
#[cfg(test)]
mod terminal_decode_once;

pub use retained_terminal::{
    PreparedRetainedNativeProviders, RetainedNativeProviderInputs, RetainedTerminalArtifact,
    TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact,
    TerminalCompilerBuiltinProposal, TerminalIeeeFloatComparisonOccurrenceProposal,
    TerminalIntegerComparisonOccurrenceProposal, TerminalNativeBoundaryApplications,
    TerminalNativeOccurrences, TerminalNativeProposalRequest, TerminalNativeProviders,
    TerminalNativeRealizationProposal, project_terminal_native_realization_proposal,
};

pub use native_admission::{
    AdmittedNativeRealization, NativeAdmissionRequest, admit_native_realization,
};
pub use provider_admission::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
};

pub use artifact_admission::{
    AdmittedNativeArtifact, AdmittedOptimizationArtifact, ArtifactLoweringError, ArtifactSections,
    TerminalPlacedViewEstablishment, VerifiedNativeArtifactInput, lower_artifact,
};
pub use boundary_application_settlement::BoundaryApplicationSettlement;
pub use callback_settlement::{
    CallbackRegistrarOccurrence, settle_callback_registrar_imports,
    };
pub use comparison_settlement::{
    settle_ieee_float_comparison_occurrences, settle_integer_comparison_occurrences,
};
pub use entry_settlement::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError,
    ValidatedNativeProgramEntrySettlement, validate_native_program_entry_settlement,
};
pub use lowering::LoweringError;
pub use optimization::{
    VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnit,
    VerifiedPsiOptimizationUnitBuildError, build_verified_psi_optimization_unit,
};
pub use provider_installation::{
    AdmittedInstalledProviderCall, AdmittedProviderInstallation, ProviderInstallationError,
    SelectedProviderAdapter,
};
