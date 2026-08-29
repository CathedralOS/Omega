#![forbid(unsafe_code)]

//! Package commands, declarations, resolution, review, and root policy
//! for Omega's registry-free package manager.
//!
//! Start with [`commands`] for complete operations, then follow the named
//! responsibility modules below. Root exports form the crate's public caller
//! surface; implementation ownership remains in the module tree.

pub mod commands;
pub mod declarations;
pub mod resolution;
pub mod review;

pub use commands::{
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, audit_package_source,
    audit_package_source_locator,
};
pub use declarations::{
    ApplicationDeclaration, BuildDeclaration, BuildDeclarationError, BuildDeclarationKind,
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildDependencyProjection, BuildFileReplacement,
    DependencyProjectionError, DependencySourceRequest, PackageDeclaration,
    PackageDeclarationError, WorkspaceDeclaration, canonical_dependency_statement,
    extract_build_declaration, extract_build_dependency_projection, extract_dependency_projection,
    extract_package_declaration, plan_dependency_addition, plan_dependency_replacement,
};
pub use omega_package_source::{
    AliasName, ExternalLocalLineage, ExternalSourceContext, GenericGitLineage, GitCommitId,
    GitExecutableIdentity, GitHubRepositoryLineage, GitLabRepositoryLineage,
    GitNetworkTransferObservation, GitObjectIdAlgorithm, GitSourceRequest, GitSourceRequestError,
    GitSourceResolutionObservation, GitTransport, GitTransportExecutableIdentity,
    GitTransportProfile, GitTreeId, IdentityError, ImmutableSourceResolution, LocalSourceLimits,
    LocalSourceResolutionObservation, PackageKey, PackageName, ResolvedGitSource,
    ResolvedLocalSnapshot, ResolvedLocalSource, SourceContentDigest, SourceLineage,
    SourceResolveError, SourceResolverStorage, WorkspaceLineageIdentity, WorkspaceMemberLineage,
    WorkspaceMemberPath, resolve_git_source_with_storage, resolve_local_source,
    resolve_local_source_snapshot_with_storage,
};
#[cfg(test)]
pub(crate) use resolution::graph::resolve_external_local_package_closure;
pub use resolution::graph::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint,
    CanonicalSourceClosureSubjectLimits, DependencyRequestPath, DependencyRequestPathStep,
    PackageClosureValidationError, PackageRootSourceRequest, PackageSourceClosureConflict,
    PackageSourceClosureConflictCandidate, PackageSourceClosureLimitKind,
    PackageSourceClosureLimits, PackageSourceClosureResolutionError, ResolveDependencySourceError,
    ResolveExternalLocalPackageClosureError, ResolveGitPackageClosureError,
    ResolveWorkspacePackageClosureError, ResolvedDependency, ResolvedDependencySourceRequest,
    ResolvedPackageClosure, ResolvedPackageNode, ResolvedPackageSourceClosure,
    ResolvedPackageSourceRequestSet, ResolvedRootPackageSourceRequest, ResolvedSourceIdentity,
    SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION, resolve_external_local_package_closure_with_storage,
    resolve_external_local_project_closure_with_storage, resolve_git_package_closure_with_storage,
    resolve_workspace_package_closure_in_context_with_storage,
    resolve_workspace_package_closure_with_storage,
};
pub use resolution::package::{
    PackageSourceCustody, ResolvePackageSourceError, ResolvedPackageSource,
    resolve_external_local_package_source_with_storage,
    resolve_external_local_project_source_with_storage, resolve_git_package_source_with_storage,
    resolve_workspace_member_package_source_with_storage,
};
#[cfg(test)]
pub(crate) use resolution::package::{
    resolve_external_local_package_source, resolve_workspace_member_package_source,
};
pub use review::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestion,
    CanonicalPackageReconstructionQuestionError, CanonicalPackageReconstructionQuestionFingerprint,
    CanonicalPackageReconstructionQuestionLimits, CompileResolvedPackageReviewsError,
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, CompilerReviewTriage,
    PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION, PackageSourcePatch, PackageSourcePatchError,
    PackageSourcePatchLimits, PackageSourcePatchSide, PackageSourceReviewCustodyRole,
    PackageSourceReviewError, PackageSourceReviewInput, PackageSourceReviewLimits,
    PackageSourceReviewRenderError, PackageSourceVerificationPhase, PackageTriageDecision,
    PackageTriageDisposition, PackageTriageReason, ReviewOnlyBaselineCapsule,
    ReviewOnlyBaselineDirectory, ReviewOnlyBaselineError, ReviewOnlyBaselineFileError,
    ReviewOnlyBaselineLimits, ReviewOnlyBaselineName, ReviewOnlyBaselineNameError,
    ReviewOnlyBaselinePackage, ReviewOnlyCandidateClosureCommitment, ReviewOnlyCanonicalRow,
    ReviewOnlyCapabilityConflict, ReviewOnlyCapabilityConflictChange,
    ReviewOnlyCapabilityConflictError, ReviewOnlyCapabilityConflictFingerprint,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyCapabilityConflictRenderError,
    ReviewOnlyCapabilityConflictSet, ReviewOnlyPackageCapabilityConflicts,
    ReviewOnlyRootPolicyDecision, ReviewOnlyRootPolicyDirectory, ReviewOnlyRootPolicyDisposition,
    ReviewOnlyRootPolicyFileError, ReviewOnlyRootPolicyName, ReviewOnlyRootPolicyNameError,
    ReviewOnlyRootPolicyRecordError, ReviewOnlyRootPolicyRecordLimits,
    ReviewOnlyRootPolicyResolution, ReviewOnlyRootPolicyResolutionCommitment,
    ReviewOnlyRootPolicyResolutionError, ReviewOnlySourceConsumptionCommitment, ReviewSetRole,
    TriageRenderError, assemble_initial_source_review, assemble_update_source_review,
    assemble_update_source_review_from_baseline, compare_review_only_capabilities,
    compare_review_only_capabilities_from_baseline, compile_resolved_package_reviews,
    package_compilation_inputs, package_compilation_inputs_for,
    recover_review_only_root_policy_resolution, render_package_source_patch,
    resolve_review_only_root_policy_decisions, triage_initial_install, triage_review_update,
    triage_review_update_from_baseline, triage_update_without_admission_baseline,
};
