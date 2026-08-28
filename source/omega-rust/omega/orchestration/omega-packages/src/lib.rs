#![forbid(unsafe_code)]

//! Exploratory package-resolution and package-admission scaffolding for the
//! Omega compiler.
//!
//! Source custody, declaration, identity, and pre-admission graph building for
//! the corrected package design. The superseded name-keyed manifest, lock,
//! install/update, and free-form review-receipt prototypes have been removed.
//! See the crate README and `TASKS_PACKAGE_MANAGER.md` before extending the
//! trust boundary.

mod capability_conflict;
mod closure_resolution;
mod compiler_handoff;
mod compiler_review;
mod conflict;
mod declaration;
mod dependency_edit;
mod dependency_projection;
mod graph;
mod identity;
mod package_reconstruction_question;
mod package_source;
mod record_file;
mod review_baseline;
mod review_closure;
mod review_evidence;
mod source;
mod source_adapter;
mod source_closure_subject;
mod source_commands;
mod source_patch;
mod source_review;
mod source_triage;

pub use capability_conflict::{
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflict,
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictFingerprint, ReviewOnlyCapabilityConflictLimits,
    ReviewOnlyCapabilityConflictRenderError, ReviewOnlyCapabilityConflictSet,
    ReviewOnlyPackageCapabilityConflicts, ReviewSetRole, compare_review_only_capabilities,
};
pub use closure_resolution::{
    DependencyRequestPath, DependencyRequestPathStep, PackageRootSourceRequest,
    PackageSourceClosureConflict, PackageSourceClosureConflictCandidate,
    PackageSourceClosureLimitKind, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    PackageSourceCustody, ResolvedDependencySourceRequest, ResolvedPackageSourceClosure,
    ResolvedPackageSourceRequestSet, ResolvedRootPackageSourceRequest,
};
pub use compiler_handoff::{package_compilation_inputs, package_compilation_inputs_for};
pub use compiler_review::{
    CompileResolvedPackageReviewsError, CompilerExecutableVerificationPhase,
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
    compile_resolved_package_reviews,
};
pub use conflict::{
    ReviewOnlyRootPolicyDecision, ReviewOnlyRootPolicyDirectory, ReviewOnlyRootPolicyDisposition,
    ReviewOnlyRootPolicyFileError, ReviewOnlyRootPolicyName, ReviewOnlyRootPolicyNameError,
    ReviewOnlyRootPolicyRecordError, ReviewOnlyRootPolicyRecordLimits,
    ReviewOnlyRootPolicyResolution, ReviewOnlyRootPolicyResolutionCommitment,
    ReviewOnlyRootPolicyResolutionError, recover_review_only_root_policy_resolution,
    resolve_review_only_root_policy_decisions,
};
pub use declaration::{
    ApplicationDeclaration, BuildDeclaration, BuildDeclarationError, BuildDeclarationKind,
    PackageDeclaration, PackageDeclarationError, WorkspaceDeclaration, extract_build_declaration,
    extract_package_declaration,
};
pub use dependency_edit::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildFileReplacement, canonical_dependency_statement,
    plan_dependency_addition, plan_dependency_replacement,
};
pub use dependency_projection::{
    BuildDependencyProjection, DependencyProjectionError, DependencySourceRequest,
    extract_build_dependency_projection, extract_dependency_projection,
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
pub use package_reconstruction_question::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestion,
    CanonicalPackageReconstructionQuestionError, CanonicalPackageReconstructionQuestionFingerprint,
    CanonicalPackageReconstructionQuestionLimits, PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION,
};
pub use package_source::{
    ResolvePackageSourceError, ResolvedPackageSource, resolve_external_local_package_source,
    resolve_external_local_project_source, resolve_git_package_source,
    resolve_workspace_member_package_source,
};
pub use review_baseline::{
    ReviewOnlyBaselineCapsule, ReviewOnlyBaselineDirectory, ReviewOnlyBaselineError,
    ReviewOnlyBaselineFileError, ReviewOnlyBaselineLimits, ReviewOnlyBaselineName,
    ReviewOnlyBaselineNameError, ReviewOnlyBaselinePackage,
    assemble_update_source_review_from_baseline, compare_review_only_capabilities_from_baseline,
    triage_review_update_from_baseline,
};
pub use review_evidence::{
    ReviewOnlyCanonicalRow, ReviewOnlyCompilerExecutableCommitment,
    ReviewOnlySourceConsumptionCommitment,
};
pub use source::{
    GitExecutableIdentity, GitNetworkTransferObservation, GitSourceRequest, GitSourceRequestError,
    GitSourceResolutionObservation, GitTransportExecutableIdentity, GitTransportProfile,
    LocalSourceLimits, ResolvedGitSource, ResolvedLocalSnapshot, ResolvedLocalSource,
    SourceResolveError, resolve_git_source, resolve_local_source, resolve_local_source_snapshot,
};
pub use source_adapter::{
    ResolveDependencySourceError, ResolveExternalLocalPackageClosureError,
    ResolveGitPackageClosureError, ResolveWorkspacePackageClosureError,
    resolve_external_local_package_closure, resolve_external_local_project_closure,
    resolve_git_package_closure, resolve_workspace_package_closure,
    resolve_workspace_package_closure_in_context,
};
pub use source_closure_subject::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint,
    CanonicalSourceClosureSubjectLimits, SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION,
};
pub use source_commands::{
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, audit_package_source,
    audit_package_source_locator,
};
pub use source_patch::{
    PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits, PackageSourcePatchSide,
    render_package_source_patch,
};
pub use source_review::{
    PackageAdvisoryRecommendation, PackageAdvisoryReviewError, PackageAdvisoryReviewOutcome,
    PackageAdvisoryReviewOutput, PackageAdvisoryReviewOutputError, PackageAdvisoryReviewRequest,
    PackageAdvisoryReviewer, PackageSourceReviewCustodyRole, PackageSourceReviewError,
    PackageSourceReviewInput, PackageSourceReviewLimits, PackageSourceReviewRenderError,
    assemble_initial_source_review, assemble_update_source_review, invoke_package_advisory_review,
};
pub use source_triage::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    TriageRenderError, triage_initial_install, triage_review_update,
    triage_update_without_admission_baseline,
};
