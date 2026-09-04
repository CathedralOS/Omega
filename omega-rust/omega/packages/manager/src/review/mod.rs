//! Compile checked review candidates, compare them, and apply root decisions.

mod audit;
pub(crate) mod baseline;
mod candidate;
mod compare;
mod decision;
pub(crate) mod reconstruction;

pub use audit::{
    CompilerReviewTriage, PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits,
    PackageSourcePatchSide, PackageSourceReviewCustodyRole, PackageSourceReviewError,
    PackageSourceReviewInput, PackageSourceReviewLimits, PackageSourceReviewRenderError,
    PackageTriageDecision, PackageTriageDisposition, PackageTriageReason, TriageRenderError,
    assemble_initial_source_review, assemble_update_source_review, render_package_source_patch,
    triage_initial_install, triage_review_update, triage_update_without_admission_baseline,
};
pub use baseline::{
    ReviewOnlyBaselineCapsule, ReviewOnlyBaselineDirectory, ReviewOnlyBaselineError,
    ReviewOnlyBaselineFileError, ReviewOnlyBaselineLimits, ReviewOnlyBaselineName,
    ReviewOnlyBaselineNameError, ReviewOnlyBaselinePackage,
    assemble_update_source_review_from_baseline, compare_review_only_capabilities_from_baseline,
    compare_review_only_root_role_from_baseline, triage_review_update_from_baseline,
};
pub use candidate::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReview,
    CompilerIssuedPackageReviewSet, ConsumerScopedSemanticBindingReviewInput,
    PackageSourceVerificationPhase, ReviewOnlyCanonicalRow, ReviewOnlySourceConsumptionCommitment,
    ReviewedPackageProductionCandidate, SemanticBindingReviewCandidate,
    compile_resolved_package_candidate_for_production,
    compile_resolved_package_candidate_for_production_with_semantic_bindings,
    compile_resolved_package_candidate_reviews, compile_resolved_package_reviews,
    compile_resolved_package_reviews_with_semantic_bindings,
};
pub use compare::{
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflict,
    ReviewOnlyCapabilityConflictBaseline, ReviewOnlyCapabilityConflictChange,
    ReviewOnlyCapabilityConflictError, ReviewOnlyCapabilityConflictFingerprint,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyCapabilityConflictRenderError,
    ReviewOnlyCapabilityConflictSet, ReviewOnlyPackageCapabilityConflicts,
    ReviewOnlyRootRoleChange, ReviewOnlyRootRoleComparisonError, ReviewOnlyRootRoleContract,
    ReviewSetRole, compare_review_only_capabilities, compare_review_only_initial_capabilities,
};
pub use decision::{
    ReviewOnlyRootPolicyDecision, ReviewOnlyRootPolicyDirectory, ReviewOnlyRootPolicyDisposition,
    ReviewOnlyRootPolicyFileError, ReviewOnlyRootPolicyName, ReviewOnlyRootPolicyNameError,
    ReviewOnlyRootPolicyRecordError, ReviewOnlyRootPolicyRecordLimits,
    ReviewOnlyRootPolicyResolution, ReviewOnlyRootPolicyResolutionCommitment,
    ReviewOnlyRootPolicyResolutionError, recover_review_only_root_policy_resolution,
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
