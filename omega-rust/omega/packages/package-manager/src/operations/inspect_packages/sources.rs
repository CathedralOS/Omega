//! Resolve pinned inspection sources and compare compiler findings to accepted policy.

use super::{PackageInspectionError, failure};
use crate::build_session::PackageBuildSession;
use crate::lock::PackageLockTarget;
use crate::operations::prepare_project::LOCAL_PROJECT_CONTEXT;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, GitResolutionOptions,
    PackageRootSourceRequest, PackageSourceClosureLimits, resolve_external_local_project_closure,
    resolve_locked_local_project_closure,
};
use crate::review::{
    CandidateReview, CompilerIssuedPackageReviewSet, PackagePolicyChangeLimits,
    PackagePolicyChangeSet, PackageSourceBindings, compare_package_policy_changes,
    review_resolved_packages,
};
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolver};
use std::path::Path;
use target::TargetProfile;

pub(super) fn review(
    project_root: &Path,
    target: TargetProfile,
    accepted: Option<&PackageLockTarget>,
    storage: &SourceResolver,
    offline: bool,
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
    let context = ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT);
    let closure = if let Some(accepted) = accepted {
        resolve_locked_local_project_closure(
            accepted.source(),
            &PackageRootSourceRequest::ExternalLocal {
                requested_root: project_root.to_path_buf(),
                source_context: context,
            },
            if offline {
                GitExactRevisionAcquisition::Offline
            } else {
                GitExactRevisionAcquisition::AllowFetch
            },
            storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .map_err(failure)?
    } else {
        resolve_external_local_project_closure(
            project_root,
            context,
            storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            GitResolutionOptions {
                pins: None,
                offline,
            },
        )
        .map_err(failure)?
    };
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
