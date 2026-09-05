use super::{PackageInspectionError, PackageInspectionOutcome, failure, report};
use crate::lock::{PackageLock, PackageLockRecoveryLimits, PackageLockTarget};
use crate::operations::PackageFileTransaction;
use crate::operations::prepare_project::LOCAL_PROJECT_CONTEXT;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, PackageRootSourceRequest,
    PackageSourceClosureLimits, resolve_external_local_project_closure_with_storage,
    resolve_locked_local_project_closure_with_storage,
};
use crate::review::{
    CompilerIssuedPackageReviewSet, PackagePolicyChangeLimits, PackagePolicyChangeSet,
    compare_package_policy_changes, compile_resolved_package_candidate_reviews,
};
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::borrow::Borrow;
use std::path::Path;
use target::TargetProfile;

const MAXIMUM_REPORT_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn inspect<Storage: Borrow<SourceResolverStorage>>(
    transaction: &PackageFileTransaction,
    requested_targets: Vec<TargetProfile>,
    details: bool,
    open_storage: impl FnOnce(&Path) -> Result<Storage, PackageInspectionError>,
) -> Result<PackageInspectionOutcome, PackageInspectionError> {
    // read_pair refuses pending commit intent. Inspection never completes a
    // publication or changes the independent install/update proposal.
    let before = transaction.read_pair().map_err(failure)?;
    let accepted = before
        .1
        .as_deref()
        .map(|bytes| {
            let text = std::str::from_utf8(bytes).map_err(failure)?;
            PackageLock::recover_text(text, PackageLockRecoveryLimits::default()).map_err(failure)
        })
        .transpose()?;
    let targets = select_targets(requested_targets, accepted.as_ref())?;
    let storage = open_storage(transaction.project_root());
    let mut outcome = PackageInspectionOutcome {
        report: String::new(),
        complete: true,
        requires_decision: false,
    };
    for target in targets {
        let baseline = accepted.as_ref().and_then(|lock| lock.target(target));
        let fresh = match &storage {
            Ok(storage) => check(
                transaction.project_root(),
                target,
                baseline,
                storage.borrow(),
            ),
            Err(error) => Err(failure(error)),
        };
        let available = match &fresh {
            Ok((source, reviews, changes)) => {
                outcome.requires_decision |= changes.requires_decision();
                Some((source, reviews, changes))
            }
            Err(_) => {
                outcome.complete = false;
                None
            }
        };
        let unavailable = fresh.as_ref().err().map(ToString::to_string);
        let section = report::render(
            target,
            baseline,
            available,
            unavailable.as_deref(),
            MAXIMUM_REPORT_BYTES.saturating_sub(outcome.report.len()),
            details,
        )
        .map_err(failure)?;
        outcome.report.try_reserve(section.len()).map_err(failure)?;
        outcome.report.push_str(&section);
    }
    let after = transaction.read_pair().map_err(failure)?;
    if after != before {
        return Err(failure(
            "project build.omg or omega.lock changed during inspection; rerun the command",
        ));
    }
    Ok(outcome)
}

fn select_targets(
    mut requested: Vec<TargetProfile>,
    accepted: Option<&PackageLock>,
) -> Result<Vec<TargetProfile>, PackageInspectionError> {
    if requested.is_empty() {
        return Ok(match accepted {
            Some(lock) => lock
                .targets()
                .iter()
                .map(PackageLockTarget::target)
                .collect(),
            None => vec![TargetProfile::host()],
        });
    }
    requested.sort_by_key(|target| target.identity().as_str());
    if requested.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(failure("duplicate inspection target"));
    }
    // An unaccepted target must be reviewed explicitly through update. Do not
    // silently resolve a new source graph when a lock is present.
    for target in &requested {
        if accepted.is_some_and(|lock| lock.target(*target).is_none()) {
            return Err(failure(format!(
                "omega.lock has no accepted target {}; use omega update --target {} for fresh review; no selector was refreshed",
                target.target_name(),
                target.target_name()
            )));
        }
    }
    Ok(requested)
}

fn check(
    project_root: &Path,
    target: TargetProfile,
    accepted: Option<&PackageLockTarget>,
    storage: &SourceResolverStorage,
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
        resolve_locked_local_project_closure_with_storage(
            accepted.source(),
            &PackageRootSourceRequest::ExternalLocal {
                requested_root: project_root.to_path_buf(),
                source_context: context,
            },
            GitExactRevisionAcquisition::AllowFetch,
            storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .map_err(failure)?
    } else {
        resolve_external_local_project_closure_with_storage(
            project_root,
            context,
            storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .map_err(failure)?
    };
    let exact = closure.for_exact_target(target);
    let reviews = compile_resolved_package_candidate_reviews(
        &exact,
        &project_root
            .join("build/package-manager")
            .join(format!("audit-{}", target.target_name())),
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
