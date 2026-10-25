//! Admit review candidates, join compiler evidence, compare and apply root decisions.

mod audit;
mod candidate;
mod compare;
mod decision;
mod restricted_build_grants;
mod timings;

pub(crate) use candidate::verify_transitive_source_custody;

pub(crate) use audit::{
    PackageSourcePatchError, PackageSourcePatchLimits, render_package_source_patch,
};
pub use candidate::CompileResolvedPackageReviewsError;
pub use candidate::PackageReviewCacheWrite;
pub(crate) use candidate::{
    CandidateReview, CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet,
    PackageSourceBindings, PackageSourceVerificationPhase, review_resolved_packages,
};
pub use candidate::{
    PackageReviewCompletion, PackageReviewComponent, PackageReviewExecutor,
    PackageReviewOccurrence, PackageReviewPass, PackageReviewRound, PackageReviewWork,
    RetainedPackageReviews,
};
pub(crate) use compare::{
    PackagePolicyChangeError, PackagePolicyChangeFingerprint, PackagePolicyChangeKind,
    PackagePolicyChangeLimits, PackagePolicyChangeSet, PackagePolicyDependencyPath,
    PackagePolicyReplacementSite, ReviewOnlyRootRoleChange, ReviewOnlyRootRoleContract,
    compare_package_policy_changes,
};
pub(crate) use decision::{
    PackagePolicyDecisionSubject, PackagePolicyResolution, PackagePolicyReviewError,
    ReviewOnlyRootPolicyDisposition, recover_package_policy_review, render_package_policy_review,
};
pub(crate) use restricted_build_grants::RestrictedBuildCheckpoint;
