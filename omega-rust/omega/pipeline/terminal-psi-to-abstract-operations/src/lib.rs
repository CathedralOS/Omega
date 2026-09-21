#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Verified Terminal-Psi admission and lowering into source-independent Omega
//! requirements.
//!
//! Enter the named responsibility modules for artifact replay, optimizer-unit
//! construction, provider-installation custody, or machine lowering.

mod artifact_admission;
mod lowering;
mod optimization;
mod provider_installation;

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
