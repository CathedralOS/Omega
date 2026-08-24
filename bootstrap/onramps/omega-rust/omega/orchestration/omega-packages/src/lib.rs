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
mod closure_resolution;
#[cfg(test)]
mod commands;
mod compiler_handoff;
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
mod source;
mod source_adapter;
mod source_commands;
#[cfg(test)]
mod update;

pub use closure_resolution::{
    DependencyRequestPath, DependencyRequestPathStep, PackageSourceClosureConflict,
    PackageSourceClosureConflictCandidate, PackageSourceClosureLimitKind,
    PackageSourceClosureLimits, PackageSourceClosureResolutionError, PackageSourceCustody,
    ResolvedPackageSourceClosure, resolve_package_source_closure,
    resolve_package_source_closure_with_limits,
};
pub use compiler_handoff::package_compilation_inputs;
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
    GitHubRepositoryLineage, GitObjectIdAlgorithm, GitTransport, GitTreeId, IdentityError,
    ImmutableSourceResolution, PackageKey, PackageName, SourceContentDigest, SourceLineage,
    WorkspaceLineageIdentity, WorkspaceMemberLineage, WorkspaceMemberPath,
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
pub use source::{
    GitSourceSpec, LocalSourceLimits, ResolvedGitSource, ResolvedLocalSnapshot,
    ResolvedLocalSource, SourceResolveError, resolve_git_source, resolve_local_source,
    resolve_local_source_snapshot,
};
pub use source_adapter::{
    ResolveDependencySourceError, ResolveWorkspacePackageClosureError,
    resolve_workspace_package_closure,
};
pub use source_commands::{
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, SourceCachePolicyCommandError,
    audit_package_source, audit_package_source_locator, resolve_source_cache_record_locator,
    write_source_cache_record_locator,
};
