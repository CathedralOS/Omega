#![forbid(unsafe_code)]

//! Package-resolution and package-admission orchestration for the Omega
//! compiler.
//!
//! This crate is intentionally about package identity, source identity,
//! manifests, lock evidence, and command workflow. Language-level evidence is
//! derived elsewhere and passed in as normalized data.

mod audit;
mod commands;
mod diff;
mod install;
mod json;
mod lock;
mod manifest;
mod resolver;
mod review;
mod source;
mod update;

pub use audit::{
    PackageGraphAudit, PackageGraphAuditError, PackageGraphAuditPackage, PackageServiceReach,
    audit_package_graph,
};
pub use commands::{
    CapabilityChangeReviewCommand, CapabilityChangeReviewCommandError, PackageGraphAuditCommand,
    PackageGraphAuditCommandError, PackageInstallPlanCommand, PackageInstallPlanCommandError,
    PackageLockUpdatePlanCommand, PackageLockUpdatePlanCommandError, PackageSourceAudit,
    PackageSourceAuditCommandError, PackageSourceRequest, PackageSourceRequestParseError,
    SourceCachePolicyCommandError, audit_package_graph_from_lock, audit_package_source,
    audit_package_source_locator, create_capability_change_review, plan_package_install_from_lock,
    plan_package_lock_update_from_lock, resolve_source_cache_record_locator,
};
pub use diff::{ManifestDelta, ManifestDiff, ManifestSeverity, diff_package_capability_manifests};
pub use install::{PackageInstallPlan, PackageInstallPlanError, plan_package_install};
pub use lock::{
    LockedDependency, LockedPackage, PackageLock, PackageLockAssemblyError, PackageLockParseError,
    PackageLockPersistenceError, PackageLockValidationError,
};
pub use manifest::{
    AliasName, BuildMachineManifest, CapabilityFlowSummary, DependencyAlias,
    InstallationBoundReach, PackageCapabilityManifest, PackageName, ProviderRequirement,
    ProviderSelection, QualificationRoute, ReproducibilityEvidence, SourceIdentity, TrustReceipt,
};
pub use resolver::{
    SourceCachePolicyRecord, SourceCacheRequest, SourceCacheVerdict, resolve_source_cache_record,
};
pub use review::{
    AcceptedManifestDelta, CAPABILITY_CHANGE_RECEIPT_SCHEMA_VERSION, CapabilityChangeReceipt,
    CapabilityChangeReceiptParseError, CapabilityChangeReceiptPersistenceError,
    CapabilityReviewError,
};
pub use source::{
    GitSourceSpec, LocalSourceLimits, ResolvedGitSource, ResolvedLocalSource, SourceResolveError,
    resolve_git_source, resolve_local_source,
};
pub use update::{
    PackageLockUpdatePlan, PackageLockUpdatePlanError, PackageUpdateAdmissionError,
    PackageUpdateDecision, decide_default_package_update, decide_reviewed_package_update,
    plan_package_lock_update,
};
