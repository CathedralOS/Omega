#![forbid(unsafe_code)]

//! Exploratory package-resolution and package-admission scaffolding for the
//! Omega compiler.
//!
//! Source custody, declaration, identity, and pre-admission graph building for
//! the corrected package design. Superseded name-keyed manifests, locks, and
//! free-form review receipts compile only in isolated crate tests; they are not
//! part of this release library's API. See the crate README and
//! `TASKS_PACKAGE_MANAGER.md` before extending the trust boundary.

#[cfg(test)]
mod audit;
mod capability_conflict;
mod closure_resolution;
#[cfg(test)]
mod commands;
mod compiler_handoff;
mod compiler_review;
mod declaration;
mod dependency_edit;
mod dependency_projection;
#[cfg(test)]
mod diff;
mod graph;
mod identity;
#[cfg(test)]
mod install;
mod json;
#[cfg(test)]
mod lock;
#[cfg(test)]
mod manifest;
mod package_source;
mod resolver;
#[cfg(test)]
mod review;
mod review_baseline;
mod review_closure;
mod review_evidence;
mod source;
mod source_adapter;
mod source_commands;
mod source_patch;
mod source_review;
mod source_triage;
#[cfg(test)]
mod update;

pub use capability_conflict::{
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflict,
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictDecision,
    ReviewOnlyCapabilityConflictDisposition, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictFingerprint, ReviewOnlyCapabilityConflictLimits,
    ReviewOnlyCapabilityConflictRenderError, ReviewOnlyCapabilityConflictSet,
    ReviewOnlyCapabilityResolution, ReviewOnlyCapabilityResolutionError,
    ReviewOnlyPackageCapabilityConflicts, ReviewSetRole, compare_review_only_capabilities,
};
pub use closure_resolution::{
    DependencyRequestPath, DependencyRequestPathStep, PackageSourceClosureConflict,
    PackageSourceClosureConflictCandidate, PackageSourceClosureLimitKind,
    PackageSourceClosureLimits, PackageSourceClosureResolutionError, PackageSourceCustody,
    ResolvedPackageSourceClosure, resolve_package_source_closure,
    resolve_package_source_closure_with_limits,
};
pub use compiler_handoff::{package_compilation_inputs, package_compilation_inputs_for};
pub use compiler_review::{
    CompileResolvedPackageReviewsError, CompilerExecutableVerificationPhase,
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
    compile_resolved_package_reviews,
};
pub use declaration::{PackageDeclaration, PackageDeclarationError, extract_package_declaration};
pub use dependency_edit::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildFileReplacement, canonical_dependency_statement,
    plan_dependency_addition, plan_dependency_replacement,
};
pub use dependency_projection::{
    DependencyProjectionError, DependencySourceRequest, extract_dependency_projection,
};
pub use graph::{
    PackageClosureValidationError, ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode,
    ResolvedSourceIdentity,
};
pub use identity::{
    AliasName, ExternalLocalLineage, ExternalSourceContext, GenericGitLineage, GitCommitId,
    GitHubRepositoryLineage, GitLabRepositoryLineage, GitObjectIdAlgorithm, GitTransport,
    GitTreeId, IdentityError, ImmutableSourceResolution, PackageKey, PackageName,
    SourceContentDigest, SourceLineage, WorkspaceLineageIdentity, WorkspaceMemberLineage,
    WorkspaceMemberPath,
};
pub use package_source::{
    ResolvePackageSourceError, ResolvedPackageSource, resolve_external_local_package_source,
    resolve_git_package_source, resolve_workspace_member_package_source,
};
pub use resolver::{
    SourceCachePolicyRecord, SourceCachePolicyRecordParseError,
    SourceCachePolicyRecordPersistenceError, SourceCacheRequest, SourceCacheVerdict,
    resolve_source_cache_record,
};
pub use review_baseline::{
    ReviewOnlyBaselineCapsule, ReviewOnlyBaselineError, ReviewOnlyBaselineLimits,
    ReviewOnlyBaselinePackage, assemble_update_source_review_from_baseline,
    compare_review_only_capabilities_from_baseline, triage_review_update_from_baseline,
};
pub use review_evidence::{
    ReviewOnlyCanonicalRow, ReviewOnlyCompilerExecutableCommitment,
    ReviewOnlySourceConsumptionCommitment,
};
pub use source::{
    GitExecutableIdentity, GitSourceRequest, GitSourceRequestError, LocalSourceLimits,
    ResolvedGitSource, ResolvedLocalSnapshot, ResolvedLocalSource, SourceResolveError,
    resolve_git_source, resolve_local_source, resolve_local_source_snapshot,
};
pub use source_adapter::{
    ResolveDependencySourceError, ResolveExternalLocalPackageClosureError,
    ResolveWorkspacePackageClosureError, resolve_external_local_package_closure,
    resolve_workspace_package_closure, resolve_workspace_package_closure_in_context,
};
pub use source_commands::{
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, SourceCachePolicyCommandError,
    audit_package_source, audit_package_source_locator, resolve_source_cache_record_locator,
    write_source_cache_record_locator,
};
pub use source_patch::{
    PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits, PackageSourcePatchSide,
    render_package_source_patch,
};
pub use source_review::{
    PackageSourceReviewCustodyRole, PackageSourceReviewError, PackageSourceReviewInput,
    PackageSourceReviewLimits, PackageSourceReviewRenderError, assemble_initial_source_review,
    assemble_update_source_review,
};
pub use source_triage::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    TriageRenderError, triage_initial_install, triage_review_update,
    triage_update_without_admission_baseline,
};
