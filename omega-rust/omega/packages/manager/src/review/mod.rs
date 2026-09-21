//! Compile checked review candidates, compare them, and apply root decisions.

mod audit;
mod candidate;
mod compare;
mod decision;
pub(crate) mod reconstruction;
mod restricted_build_grants;
mod symbolic_boundary_applications;

pub(crate) use candidate::{
    compile_resolved_package_candidate_for_check,
    compile_resolved_package_candidate_for_check_with_checkpoint, verify_transitive_source_custody,
};

pub use audit::{
    CompilerReviewTriage, PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits,
    PackageSourcePatchSide, PackageSourceReviewCustodyRole, PackageSourceReviewError,
    PackageSourceReviewInput, PackageSourceReviewLimits, PackageSourceReviewRenderError,
    PackageTriageDecision, PackageTriageDisposition, PackageTriageReason, TriageRenderError,
    assemble_initial_source_review, assemble_update_source_review, render_package_source_patch,
    triage_initial_install, triage_review_update, triage_update_without_admission_baseline,
};
pub use candidate::{
    CandidateSourcePreparation, CompileResolvedPackageReviewsError, CompilerIssuedPackageReview,
    CompilerIssuedPackageReviewSet, ConsumerScopedSemanticBindingReviewInput,
    PackageSourceVerificationPhase, ReviewOnlySourceConsumptionCommitment,
    ReviewedPackageProductionCandidate, SemanticBindingReview, SemanticBindingReviewCandidate,
    compile_resolved_package_candidate_for_production,
    compile_resolved_package_candidate_for_production_with_checkpoint,
    compile_resolved_package_reviews, compile_resolved_package_reviews_reusing,
    compile_resolved_package_reviews_with_checkpoint,
};
pub use compare::{
    LockedPolicyComparisonError, PackagePolicyChangeError, PackagePolicyChangeFingerprint,
    PackagePolicyChangeKind, PackagePolicyChangeLimits, PackagePolicyChangeSet,
    PackagePolicyDependencyPath, PackagePolicyDependencyPathStep, PackagePolicyPackageChange,
    PackagePolicyReplacementSite, PackagePolicyRowChange, PackagePolicySourceReplacement,
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflict,
    ReviewOnlyCapabilityConflictBaseline, ReviewOnlyCapabilityConflictChange,
    ReviewOnlyCapabilityConflictError, ReviewOnlyCapabilityConflictFingerprint,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyCapabilityConflictRenderError,
    ReviewOnlyCapabilityConflictSet, ReviewOnlyPackageCapabilityConflicts,
    ReviewOnlyRootRoleChange, ReviewOnlyRootRoleContract, ReviewSetRole,
    compare_locked_package_policies, compare_package_policy_changes,
    compare_review_only_capabilities, compare_review_only_initial_capabilities,
};
pub use decision::{
    PackagePolicyDecision, PackagePolicyDecisionError, PackagePolicyDecisionSubject,
    PackagePolicyResolution, PackagePolicyReviewError, ReviewOnlyRootPolicyDecision,
    ReviewOnlyRootPolicyDisposition, ReviewOnlyRootPolicyRecordError,
    ReviewOnlyRootPolicyRecordLimits, ReviewOnlyRootPolicyResolution,
    ReviewOnlyRootPolicyResolutionCommitment, ReviewOnlyRootPolicyResolutionError,
    recover_package_policy_review, recover_review_only_root_policy_resolution,
    render_package_policy_review, resolve_package_policy_decisions,
    resolve_review_only_root_policy_decisions,
};
pub use reconstruction::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestion,
    CanonicalPackageReconstructionQuestionError, CanonicalPackageReconstructionQuestionFingerprint,
    CanonicalPackageReconstructionQuestionLimits, FreshPackageRootPolicyAcceptance,
    FreshPackageRootPolicyError, LocallyComposedPackageObligationEntry,
    LocallyComposedPackageObligationResults, PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION,
    bind_fresh_package_root_policy,
};
pub use restricted_build_grants::{
    RestrictedBuildCheckpoint, UngrantedRestrictedBuildRequest, ungranted_restricted_build_requests,
};
pub use symbolic_boundary_applications::{
    ClosedSuppliedBoundaryApplicationDemand, ClosedSuppliedBoundaryApplicationDemands,
    ClosedSuppliedBoundaryApplicationSource, ConcreteProducerBinderCategory,
    ConcreteProducerTypeSpecialization, ConcreteProducerTypeSubstitution,
    SymbolicBoundaryApplicationClosureError, SymbolicBoundaryApplicationClosureRequest,
    close_supplied_reviewed_symbolic_boundary_applications,
};
