//! Compile checked review candidates, compare them, and apply root decisions.

mod audit;
mod candidate;
mod compare;
mod decision;
mod restricted_build_grants;
mod timings;

pub(crate) use candidate::verify_transitive_source_custody;

pub use audit::{
    CompilerReviewTriage, PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits,
    PackageSourcePatchSide, PackageSourceReviewCustodyRole, PackageSourceReviewError,
    PackageSourceReviewInput, PackageSourceReviewLimits, PackageSourceReviewRenderError,
    PackageTriageDecision, PackageTriageDisposition, PackageTriageReason, TriageRenderError,
    assemble_initial_source_review, assemble_update_source_review, render_package_source_patch,
    triage_initial_install, triage_review_update, triage_update_without_admission_baseline,
};
pub use candidate::{
    CandidateCompilation, CandidateSourcePreparation, CompileResolvedPackageReviewsError,
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet,
    ConsumerScopedSemanticBindingReviewInput, PackageSourceVerificationPhase,
    ReviewOnlySourceConsumptionCommitment, SemanticBindingReview, SemanticBindingReviewCandidate,
    compile_resolved_package_reviews, compile_resolved_package_reviews_reusing,
};
pub use compare::{
    LockedPolicyComparisonError, PackagePolicyChangeError, PackagePolicyChangeFingerprint,
    PackagePolicyChangeKind, PackagePolicyChangeLimits, PackagePolicyChangeSet,
    PackagePolicyDependencyPath, PackagePolicyDependencyPathStep, PackagePolicyPackageChange,
    PackagePolicyReplacementSite, PackagePolicyRowChange, PackagePolicySourceReplacement,
    ReviewOnlyRootRoleChange, ReviewOnlyRootRoleContract, compare_locked_package_policies,
    compare_package_policy_changes,
};
pub use decision::{
    PackagePolicyDecision, PackagePolicyDecisionError, PackagePolicyDecisionSubject,
    PackagePolicyResolution, PackagePolicyReviewError, ReviewOnlyRootPolicyDisposition,
    recover_package_policy_review, render_package_policy_review, resolve_package_policy_decisions,
};
pub use restricted_build_grants::{
    RestrictedBuildCheckpoint, UngrantedRestrictedBuildRequest, ungranted_restricted_build_requests,
};
