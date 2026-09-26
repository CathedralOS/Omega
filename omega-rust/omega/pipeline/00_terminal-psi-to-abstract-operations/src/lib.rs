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

mod artifact_admission;
mod entry_settlement;
mod lowering;
mod optimization;
mod provider_installation;

pub use entry_settlement::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError,
    ValidatedNativeProgramEntrySettlement, validate_native_program_entry_settlement,
    validate_fused_program_entry_establishments,
};
pub use artifact_admission::{
    AdmittedNativeArtifact, AdmittedOptimizationArtifact, ArtifactLoweringError, ArtifactSections,
    TerminalPlacedViewEstablishment, VerifiedNativeArtifactInput, lower_artifact,
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
