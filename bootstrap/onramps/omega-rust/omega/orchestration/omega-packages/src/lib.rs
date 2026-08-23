#![forbid(unsafe_code)]

//! Package-resolution and package-admission orchestration for the Omega
//! compiler.
//!
//! This crate is intentionally about package identity, source identity,
//! manifests, lock evidence, and command workflow. Language-level evidence is
//! derived elsewhere and passed in as normalized data.

mod commands;
mod diff;
mod lock;
mod manifest;
mod review;
mod source;

pub use commands::{PackageSourceAudit, PackageSourceRequest, audit_package_source};
pub use diff::{ManifestDelta, ManifestDiff, ManifestSeverity, diff_package_capability_manifests};
pub use lock::{LockedDependency, LockedPackage, PackageLock};
pub use manifest::{
    AliasName, BuildMachineManifest, CapabilityFlowSummary, DependencyAlias,
    InstallationBoundReach, PackageCapabilityManifest, PackageName, ProviderRequirement,
    ProviderSelection, QualificationRoute, ReproducibilityEvidence, SourceIdentity, TrustReceipt,
};
pub use review::{
    AcceptedManifestDelta, CAPABILITY_CHANGE_RECEIPT_SCHEMA_VERSION, CapabilityChangeReceipt,
    CapabilityReviewError,
};
pub use source::{
    GitSourceSpec, LocalSourceLimits, ResolvedGitSource, ResolvedLocalSource, SourceResolveError,
    resolve_git_source, resolve_local_source,
};
