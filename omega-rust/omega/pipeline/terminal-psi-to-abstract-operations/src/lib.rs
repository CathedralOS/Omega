#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Verified Terminal-Psi admission and lowering into source-independent Omega
//! requirements.
//!
//! Enter the named responsibility modules for artifact replay, optimizer-unit
//! construction, provider-installation custody, or machine lowering.

mod artifact;
mod lowering;
mod optimization;
mod provider_installation;
mod shared;

pub use artifact::{
    ArtifactLoweringError, VerifiedNativeArtifactInput,
    VerifiedNativeArtifactInputWithPlacedViewInputs, lower_artifact_sections,
    lower_artifact_sections_for_native_realization,
    lower_artifact_sections_for_native_realization_with_placed_view_inputs,
    lower_artifact_sections_for_optimization, lower_artifact_sections_with_placed_view_inputs,
    lower_replay_artifact_sections, lower_replay_artifact_sections_for_optimization,
    lower_replay_artifact_sections_with_placed_view_inputs,
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
