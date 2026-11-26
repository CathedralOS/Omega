/*
    We admit package review candidates, join compiler-issued evidence to the
    resolved immutable source closure, compare normalized policy rows against
    the accepted baseline, and apply root-project policy decisions.
*/

/*
    Package review orchestration belongs to the package manager's review
    boundary (`review/mod.rs`).
*/

mod audit;
mod candidate;
mod compare;
mod decision;
mod restricted_build_grants;
pub(crate) mod timings;

pub(crate) use candidate::verify_transitive_source_custody;

pub(crate) use audit::{PackageSourcePatchError, render_package_source_patch};
pub(crate) use candidate::{
    CandidateReview, CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet,
    PackageSourceBindings, review_resolved_packages,
};
pub use candidate::{
    CompileResolvedPackageReviewCause, CompileResolvedPackageReviewsError,
    PackageSourceVerificationPhase,
};
pub(crate) use compare::{
    PackagePolicyChangeError, PackagePolicyChangeFingerprint, PackagePolicyChangeKind,
    PackagePolicyChangeLimits, PackagePolicyChangeSet, PackagePolicyDependencyPath,
    PackagePolicyReplacementSite, compare_package_policy_changes,
};
pub(crate) use decision::{
    PackagePolicyDecisionSubject, PackagePolicyResolution, PackagePolicyReviewError,
    ReviewOnlyRootPolicyDisposition, recover_package_policy_review, render_build_lineage,
    render_build_request, render_package_policy_review,
};
pub(crate) use restricted_build_grants::OccurrenceBuildGrants;
pub use restricted_build_grants::UngrantedRestrictedBuildRequest;

pub(crate) mod build_session;
mod inspect_packages;
pub mod inspect_source;
mod package_change;
mod rendering;

pub use inspect_packages::{
    PackageInspectionError, PackageInspectionOptions, PackageInspectionOutcome,
    PackageInspectionSources, PackageInspectionTransaction, PreparedPackageInspection,
};
pub(crate) use package_change::{
    PackageChangeError, PackageChangeReview, review_package_change_reusing,
};
