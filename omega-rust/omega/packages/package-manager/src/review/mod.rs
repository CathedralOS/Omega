//! Compile checked review candidates, compare them, and apply root decisions.

mod audit;
mod candidate;
mod compare;
mod decision;
mod restricted_build_grants;
mod timings;

pub(crate) use candidate::verify_transitive_source_custody;

pub use audit::{
    CompilerReviewTriage, PackageSourceReviewCustodyRole, PackageSourceReviewError,
    PackageSourceReviewInput, PackageSourceReviewLimits, PackageSourceReviewRenderError,
    PackageTriageDecision, PackageTriageDisposition, PackageTriageReason, TriageRenderError,
    assemble_initial_source_review, assemble_update_source_review,
    triage_initial_install, triage_review_update, triage_update_without_admission_baseline,
};
pub(crate) use audit::{ PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits, render_package_source_patch};
pub use candidate::{
    CandidateCompilation, CompileResolvedPackageReviewsError,
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet,
    ConsumerScopedSemanticBindingReviewInput, PackageSourceVerificationPhase,
    ReviewOnlySourceConsumptionCommitment, SemanticBindingReview, SemanticBindingReviewCandidate,
    compile_resolved_package_reviews, compile_resolved_package_reviews_reusing,
};
pub(crate) use candidate::{ CandidateSourcePreparation};
pub use compare::{
    LockedPolicyComparisonError, compare_locked_package_policies,
};
pub(crate) use compare::{ PackagePolicyChangeError, PackagePolicyChangeFingerprint,
    PackagePolicyChangeKind, PackagePolicyChangeLimits, PackagePolicyChangeSet,
    PackagePolicyDependencyPath,
    PackagePolicyReplacementSite,
    ReviewOnlyRootRoleChange, ReviewOnlyRootRoleContract,
    compare_package_policy_changes};
pub(crate) use decision::{ PackagePolicyDecisionSubject,
    PackagePolicyResolution, PackagePolicyReviewError, ReviewOnlyRootPolicyDisposition,
    recover_package_policy_review, render_package_policy_review};
pub(crate) use restricted_build_grants::{RestrictedBuildCheckpoint};
