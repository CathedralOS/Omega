#![forbid(unsafe_code)]

//! Package-resolution and package-admission orchestration for the Omega
//! compiler.
//!
//! This crate is intentionally about package identity, source identity,
//! manifests, lock evidence, and command workflow. Language-level evidence is
//! derived elsewhere and passed in as normalized data.

mod diff;
mod manifest;
mod source;

pub use diff::{ManifestDelta, ManifestDiff, ManifestSeverity, diff_package_capability_manifests};
pub use manifest::{
    AliasName, BuildMachineManifest, CapabilityFlowSummary, DependencyAlias,
    InstallationBoundReach, PackageCapabilityManifest, PackageName, ProviderRequirement,
    ProviderSelection, QualificationRoute, ReproducibilityEvidence, SourceIdentity, TrustReceipt,
};
pub use source::{
    LocalSourceLimits, ResolvedLocalSource, SourceResolveError, resolve_local_source,
};
