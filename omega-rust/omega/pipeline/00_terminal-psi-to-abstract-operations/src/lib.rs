#![forbid(unsafe_code)]

//! Terminal Psi to abstract operations: the first Omega program stage.
//!
//! The stage operation is `lower_artifact` (`artifact_admission`). It replays
//! and verifies a Terminal artifact's semantic and proof sections, then lowers
//! each machine (`lowering`) into source-independent abstract operations.
//! Two other entrances serve the same verified input:
//! `build_verified_psi_optimization_unit` (`optimization`) builds the Psi
//! optimization unit the optimizer rewrites, and `admit_provider_installation`
//! (`provider_installation`) joins provider installation custody to the
//! admitted artifact.
//!
//! `entry_settlement` and `callback_settlement` admit native entry sidecars
//! against already-produced Terminal artifacts. These checks do not create
//! another program stage or grant artifact-publication authority.
//! `comparison_settlement` rejoins emitted comparison occurrences to authored
//! applications and this target's selected providers. It produces source-free
//! comparison records, not a substitute for canonical artifact admission.

mod artifact_admission;
mod callback_settlement;
mod comparison_settlement;
mod entry_settlement;
mod lowering;
mod optimization;
mod provider_installation;

pub use artifact_admission::{
    AdmittedNativeArtifact, AdmittedOptimizationArtifact, ArtifactLoweringError, ArtifactSections,
    TerminalPlacedViewEstablishment, VerifiedNativeArtifactInput, lower_artifact,
};
pub use callback_settlement::{
    CallbackRegistrarOccurrence, settle_callback_registrar_imports,
    validate_callback_thunk_settlement,
};
pub use comparison_settlement::{
    settle_ieee_float_comparison_occurrences, settle_integer_comparison_occurrences,
};
pub use entry_settlement::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError,
    ValidatedNativeProgramEntrySettlement, validate_fused_program_entry_establishments,
    validate_native_program_entry_settlement,
};
pub use lowering::LoweringError;
pub use optimization::{
    VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnit,
    VerifiedPsiOptimizationUnitBuildError, build_verified_psi_optimization_unit,
};
pub use provider_installation::{
    AdmittedInstalledProviderCall, AdmittedProviderInstallation, ProviderInstallationError,
    SelectedProviderAdapter, admit_provider_installation,
};
