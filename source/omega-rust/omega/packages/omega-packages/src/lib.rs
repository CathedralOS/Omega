#![forbid(unsafe_code)]

//! Package declarations, immutable source resolution, closure review, and root
//! policy for Omega's registry-free package manager.
//!
//! Start with the responsibility modules below. The crate keeps its historical
//! flat exports for callers, while implementation ownership follows the module
//! tree instead of accumulating in this root.

pub mod declarations;
pub mod resolution;
pub mod review;
mod storage;

// Preserve the established crate-internal source boundary while the external
// facade below remains an explicit compatibility list.
pub(crate) use resolution::source;

pub use declarations::{
    ApplicationDeclaration, BuildDeclaration, BuildDeclarationError, BuildDeclarationKind,
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildDependencyProjection, BuildFileReplacement,
    DependencyProjectionError, DependencySourceRequest, PackageDeclaration,
    PackageDeclarationError, WorkspaceDeclaration, canonical_dependency_statement,
    extract_build_declaration, extract_build_dependency_projection, extract_dependency_projection,
    extract_package_declaration, plan_dependency_addition, plan_dependency_replacement,
};
pub use resolution::{
    AliasName, CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint,
    CanonicalSourceClosureSubjectLimits, DependencyRequestPath, DependencyRequestPathStep,
    ExternalLocalLineage, ExternalSourceContext, GenericGitLineage, GitCommitId,
    GitExecutableIdentity, GitHubRepositoryLineage, GitLabRepositoryLineage,
    GitNetworkTransferObservation, GitObjectIdAlgorithm, GitSourceRequest, GitSourceRequestError,
    GitSourceResolutionObservation, GitTransport, GitTransportExecutableIdentity,
    GitTransportProfile, GitTreeId, IdentityError, ImmutableSourceResolution, LocalSourceLimits,
    PackageClosureValidationError, PackageKey, PackageName, PackageRootSourceRequest,
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceClosureConflict,
    PackageSourceClosureConflictCandidate, PackageSourceClosureLimitKind,
    PackageSourceClosureLimits, PackageSourceClosureResolutionError, PackageSourceCustody,
    PackageSourceRequest, PackageSourceRequestParseError, ResolveDependencySourceError,
    ResolveExternalLocalPackageClosureError, ResolveGitPackageClosureError,
    ResolvePackageSourceError, ResolveWorkspacePackageClosureError, ResolvedDependency,
    ResolvedDependencySourceRequest, ResolvedGitSource, ResolvedLocalSnapshot, ResolvedLocalSource,
    ResolvedPackageClosure, ResolvedPackageNode, ResolvedPackageSource,
    ResolvedPackageSourceClosure, ResolvedPackageSourceRequestSet,
    ResolvedRootPackageSourceRequest, ResolvedSourceIdentity,
    SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION, SourceAdapter, SourceContentDigest, SourceLineage,
    SourceResolveError, SourceResolverStorage, WorkspaceLineageIdentity, WorkspaceMemberLineage,
    WorkspaceMemberPath, audit_package_source, audit_package_source_locator,
    resolve_external_local_package_closure, resolve_external_local_package_closure_with_storage,
    resolve_external_local_package_source, resolve_external_local_project_closure,
    resolve_external_local_project_closure_with_storage, resolve_external_local_project_source,
    resolve_git_package_closure, resolve_git_package_closure_with_storage,
    resolve_git_package_source, resolve_git_source, resolve_local_source,
    resolve_local_source_snapshot, resolve_workspace_member_package_source,
    resolve_workspace_package_closure, resolve_workspace_package_closure_in_context,
    resolve_workspace_package_closure_in_context_with_storage,
    resolve_workspace_package_closure_with_storage,
};
pub use review::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestion,
    CanonicalPackageReconstructionQuestionError, CanonicalPackageReconstructionQuestionFingerprint,
    CanonicalPackageReconstructionQuestionLimits, CompileResolvedPackageReviewsError,
    CompilerExecutableVerificationPhase, CompilerIssuedPackageReview,
    CompilerIssuedPackageReviewSet, CompilerReviewTriage,
    PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION, PackageAdvisoryRecommendation,
    PackageAdvisoryReviewError, PackageAdvisoryReviewOutcome, PackageAdvisoryReviewOutput,
    PackageAdvisoryReviewOutputError, PackageAdvisoryReviewRequest, PackageAdvisoryReviewer,
    PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits, PackageSourcePatchSide,
    PackageSourceReviewCustodyRole, PackageSourceReviewError, PackageSourceReviewInput,
    PackageSourceReviewLimits, PackageSourceReviewRenderError, PackageSourceVerificationPhase,
    PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    ReviewOnlyBaselineCapsule, ReviewOnlyBaselineDirectory, ReviewOnlyBaselineError,
    ReviewOnlyBaselineFileError, ReviewOnlyBaselineLimits, ReviewOnlyBaselineName,
    ReviewOnlyBaselineNameError, ReviewOnlyBaselinePackage, ReviewOnlyCandidateClosureCommitment,
    ReviewOnlyCanonicalRow, ReviewOnlyCapabilityConflict, ReviewOnlyCapabilityConflictChange,
    ReviewOnlyCapabilityConflictError, ReviewOnlyCapabilityConflictFingerprint,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyCapabilityConflictRenderError,
    ReviewOnlyCapabilityConflictSet, ReviewOnlyCompilerExecutableCommitment,
    ReviewOnlyPackageCapabilityConflicts, ReviewOnlyRootPolicyDecision,
    ReviewOnlyRootPolicyDirectory, ReviewOnlyRootPolicyDisposition, ReviewOnlyRootPolicyFileError,
    ReviewOnlyRootPolicyName, ReviewOnlyRootPolicyNameError, ReviewOnlyRootPolicyRecordError,
    ReviewOnlyRootPolicyRecordLimits, ReviewOnlyRootPolicyResolution,
    ReviewOnlyRootPolicyResolutionCommitment, ReviewOnlyRootPolicyResolutionError,
    ReviewOnlySourceConsumptionCommitment, ReviewSetRole, TriageRenderError,
    assemble_initial_source_review, assemble_update_source_review,
    assemble_update_source_review_from_baseline, compare_review_only_capabilities,
    compare_review_only_capabilities_from_baseline, compile_resolved_package_reviews,
    invoke_package_advisory_review, package_compilation_inputs, package_compilation_inputs_for,
    recover_review_only_root_policy_resolution, render_package_source_patch,
    resolve_review_only_root_policy_decisions, triage_initial_install, triage_review_update,
    triage_review_update_from_baseline, triage_update_without_admission_baseline,
};
