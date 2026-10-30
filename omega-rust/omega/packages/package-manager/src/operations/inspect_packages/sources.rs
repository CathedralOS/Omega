//! Compare executed inspection sources and compiler findings to accepted policy.

use super::{PackageInspectionError, failure};
use crate::build_session::PackageBuildSession;
use crate::lock::PackageLockTarget;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ResolvedPackageSourceClosure, mutable_local_graph_matches,
};
use crate::review::{
    CandidateReview, CompilerIssuedPackageReviewSet, PackagePolicyChangeLimits,
    PackagePolicyChangeSet, PackageSourceBindings, compare_package_policy_changes,
    review_resolved_packages,
};
use target::TargetProfile;

/// An executed source closure bound to the inspection's original project.
pub struct PackageInspectionSources {
    pub(super) closure: ResolvedPackageSourceClosure,
    pub(super) identity: std::sync::Arc<super::InspectionIdentity>,
}

pub(super) fn review(
    sources: &PackageInspectionSources,
    target: TargetProfile,
    accepted: Option<&PackageLockTarget>,
    root_build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
    session: &mut PackageBuildSession,
    source_bindings: &mut PackageSourceBindings,
    execute: &mut crate::review::PackageReviewExecutor<'_>,
) -> Result<
    (
        CanonicalSourceClosureSubject,
        CompilerIssuedPackageReviewSet,
        PackagePolicyChangeSet,
    ),
    PackageInspectionError,
> {
    let closure = &sources.closure;
    if let Some(accepted) = accepted
        && !mutable_local_graph_matches(
            accepted.source(),
            closure,
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .map_err(failure)?
    {
        return Err(failure(
            "executed source graph differs from the accepted graph; use omega update for fresh review; no selector was refreshed",
        ));
    }
    let exact = closure.for_exact_target(target);
    let reviews = review_resolved_packages(
        CandidateReview {
            target_closure: &exact,
            root_build_snapshot,
        },
        session,
        source_bindings,
        execute,
    )
    .map_err(failure)?;
    let changes = compare_package_policy_changes(
        accepted,
        &reviews,
        &exact,
        PackagePolicyChangeLimits::default(),
    )
    .map_err(failure)?;
    let source = CanonicalSourceClosureSubject::from_resolved(
        &exact,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .map_err(failure)?;
    Ok((source, reviews, changes))
}
