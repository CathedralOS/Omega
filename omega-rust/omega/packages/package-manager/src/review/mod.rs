//! Compile checked review candidates, compare them, and apply root decisions.

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
pub(crate) use candidate::{
    CandidateCompilation, CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet,
    PackageSourceBindings, PackageSourceVerificationPhase, SemanticBindingReview,
    compile_resolved_package_reviews_reusing,
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
