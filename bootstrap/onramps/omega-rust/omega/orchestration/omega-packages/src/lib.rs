#![forbid(unsafe_code)]

//! Exploratory package-resolution and package-admission scaffolding for the
//! Omega compiler.
//!
//! The current name-keyed identities, caller-constructible manifests,
//! standalone JSON persistence, and free-form review receipts predate the
//! corrected package design and are not production trust boundaries. See this
//! crate's README and `TASKS_PACKAGE_MANAGER.md` before reusing an API.

mod audit;
mod commands;
mod declaration;
mod diff;
mod identity;
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
    PackageGraphAuditCommandError, PackageGraphAuditFromPathsCommandError,
    PackageInstallPlanCommand, PackageInstallPlanCommandError, PackageLockAssemblyCommand,
    PackageLockAssemblyFromPathsCommandError, PackageLockUpdatePlanCommand,
    PackageLockUpdatePlanCommandError, PackageSourceAudit, PackageSourceAuditCommandError,
    PackageSourceRequest, PackageSourceRequestParseError, SourceAdapter,
    SourceCachePolicyCommandError, assemble_package_lock_from_paths, audit_package_graph_from_lock,
    audit_package_graph_from_paths, audit_package_source, audit_package_source_locator,
    create_capability_change_review, plan_package_install_from_lock,
    plan_package_lock_update_from_lock, resolve_source_cache_record_locator,
    write_source_cache_record_locator,
};
pub use declaration::{PackageDeclaration, PackageDeclarationError, extract_package_declaration};
pub use diff::{ManifestDelta, ManifestDiff, ManifestSeverity, diff_package_capability_manifests};
pub use identity::PackageName;
pub use install::{PackageInstallPlan, PackageInstallPlanError, plan_package_install};
pub use lock::{
    LockedDependency, LockedPackage, PackageLock, PackageLockAssemblyError, PackageLockParseError,
    PackageLockPersistenceError, PackageLockValidationError,
};
pub use manifest::{
    AliasName, BuildMachineManifest, CapabilityFlowSummary, DependencyAlias,
    InstallationBoundReach, PackageCapabilityManifest, PackageCapabilityManifestParseError,
    PackageCapabilityManifestPersistenceError, ProviderRequirement, ProviderSelection,
    QualificationRoute, ReproducibilityEvidence, SourceIdentity, TrustReceipt,
};
pub use resolver::{
    SourceCachePolicyRecord, SourceCachePolicyRecordParseError,
    SourceCachePolicyRecordPersistenceError, SourceCacheRequest, SourceCacheVerdict,
    resolve_source_cache_record,
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
