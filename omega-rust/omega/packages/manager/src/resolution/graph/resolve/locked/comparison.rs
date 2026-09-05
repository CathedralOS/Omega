//! Pure joins performed before any dependent acquisition.

use super::{ResolveLockedPackageClosureError as Error, RootPolicy};
use crate::declarations::dependencies::read::DependencySourceRequest;
use crate::resolution::graph::{
    CanonicalDependencySourceRequest as RecordedRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    PackageRootSourceRequest, PackageSourceClosureLimits, ResolvedPackageSourceClosure,
    ResolvedSourceIdentity,
};
use crate::resolution::source::PackageSourceCustody;

pub(super) fn root_request_matches(
    recorded: &CanonicalRootSourceRequest,
    actual: &PackageRootSourceRequest,
) -> bool {
    match (recorded, actual) {
        (
            CanonicalRootSourceRequest::Git {
                requested_locator,
                requested_revision,
                selection,
            },
            PackageRootSourceRequest::Git(actual),
        ) => {
            requested_locator == actual.acquisition().requested_locator()
                && requested_revision == actual.acquisition().requested_revision()
                && selection == actual.selection()
        }
        (
            CanonicalRootSourceRequest::ExternalLocal {
                requested_root,
                source_context,
            },
            PackageRootSourceRequest::ExternalLocal {
                requested_root: actual,
                source_context: context,
            },
        ) => requested_root == actual.as_os_str().as_encoded_bytes() && source_context == context,
        (
            CanonicalRootSourceRequest::WorkspaceMember {
                workspace_root_source,
                member_path,
                requested_workspace_root,
            },
            PackageRootSourceRequest::WorkspaceMember {
                workspace_root_source: actual_source,
                member_path: actual_member,
                requested_workspace_root: actual_root,
            },
        ) => {
            workspace_root_source == actual_source
                && member_path == actual_member
                && requested_workspace_root == actual_root.as_os_str().as_encoded_bytes()
        }
        _ => false,
    }
}

pub(super) fn limits(
    subject: &CanonicalSourceClosureSubject,
    closure: PackageSourceClosureLimits,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), Error> {
    if subject.packages().len() > closure.max_packages
        || subject.packages().len() > limits.maximum_packages
        || subject.dependency_requests().len() > closure.max_dependency_requests
        || subject.dependency_requests().len() > limits.maximum_dependency_requests
        || subject.canonical_bytes().len() > limits.maximum_record_bytes
    {
        return Err(Error::LimitExceeded);
    }
    subject.validate_recovery_limits(limits)?;
    Ok(())
}

pub(super) fn custody(
    subject: &CanonicalSourceClosureSubject,
    expected: &ResolvedSourceIdentity,
    actual: &PackageSourceCustody,
) -> Result<(), Error> {
    checked_custody(subject, expected, actual, false)
}

pub(super) fn requester_custody(
    subject: &CanonicalSourceClosureSubject,
    expected: &ResolvedSourceIdentity,
    actual: &PackageSourceCustody,
    policy: RootPolicy,
) -> Result<(), Error> {
    checked_custody(
        subject,
        expected,
        actual,
        matches!(policy, RootPolicy::MutableLocal)
            && expected.key() == subject.root().selected().key(),
    )
}

fn checked_custody(
    subject: &CanonicalSourceClosureSubject,
    expected: &ResolvedSourceIdentity,
    actual: &PackageSourceCustody,
    mutable_root: bool,
) -> Result<(), Error> {
    if actual.key() != expected.key()
        || (!mutable_root && actual.resolution() != expected.resolution())
    {
        return Err(Error::mismatch(
            expected.key(),
            "fresh source key or immutable content differs",
        ));
    }
    if subject.package_navigation(expected.key()) != Some(actual.navigation()) {
        return Err(Error::mismatch(
            expected.key(),
            "fresh source navigation differs",
        ));
    }
    if subject.package_dependency_projection(expected.key())
        != Some(actual.projected_dependencies())
    {
        return Err(Error::mismatch(
            expected.key(),
            "complete fresh dependency projection differs",
        ));
    }
    Ok(())
}

pub(super) fn edge<'a>(
    subject: &'a CanonicalSourceClosureSubject,
    requester: &PackageSourceCustody,
    ordinal: usize,
    request: &DependencySourceRequest,
    policy: RootPolicy,
) -> Result<&'a CanonicalDependencySourceSelection, Error> {
    let package_index = subject
        .packages()
        .binary_search_by(|source| source.key().cmp(requester.key()))
        .map_err(|_| Error::mismatch(requester.key(), "requester is absent from locked graph"))?;
    requester_custody(
        subject,
        &subject.packages()[package_index],
        requester,
        policy,
    )?;
    let index = subject
        .dependency_requests()
        .binary_search_by(|edge| {
            edge.requester()
                .cmp(requester.key())
                .then(edge.dependency_index().cmp(&ordinal))
        })
        .map_err(|_| {
            Error::mismatch(
                requester.key(),
                "authored request occurrence is absent from locked graph",
            )
        })?;
    let edge = &subject.dependency_requests()[index];
    if !request_matches(edge.request(), request) {
        return Err(Error::mismatch(
            requester.key(),
            "exact authored dependency request differs",
        ));
    }
    Ok(edge)
}

pub(super) fn local_root_context_matches(
    recorded: &CanonicalRootSourceRequest,
    actual: &PackageRootSourceRequest,
) -> bool {
    matches!((recorded, actual), (
        CanonicalRootSourceRequest::ExternalLocal { source_context, .. },
        PackageRootSourceRequest::ExternalLocal { source_context: actual, .. },
    ) if source_context == actual)
}

/// Compare typed graph fields; never rewrite or re-encode a recovered record
/// to manufacture a subject matching the edited root.
pub(super) fn mutable_local_graph_matches(
    recorded: &CanonicalSourceClosureSubject,
    closure: &ResolvedPackageSourceClosure,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<bool, Error> {
    let fresh = CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(recorded.target_profile()),
        limits,
    )?;
    let root_key = recorded.root().selected().key();
    Ok(local_root_context_matches(
        recorded.root().request(),
        closure.source_requests().root().request(),
    ) && recorded.target_profile() == fresh.target_profile()
        && recorded.root_role() == fresh.root_role()
        && root_key == fresh.root().selected().key()
        && recorded.packages().len() == fresh.packages().len()
        && recorded
            .packages()
            .iter()
            .zip(fresh.packages())
            .all(|(before, after)| {
                before.key() == after.key()
                    && (before.key() == root_key || before.resolution() == after.resolution())
                    && recorded.package_navigation(before.key())
                        == fresh.package_navigation(after.key())
                    && recorded.package_dependency_projection(before.key())
                        == fresh.package_dependency_projection(after.key())
            })
        && recorded.dependency_requests() == fresh.dependency_requests())
}

fn request_matches(recorded: &RecordedRequest, actual: &DependencySourceRequest) -> bool {
    match (recorded, actual) {
        (
            RecordedRequest::Path {
                explicit_alias,
                location,
            },
            DependencySourceRequest::Path {
                explicit_alias: alias,
                location: path,
            },
        ) => explicit_alias == alias && location == path,
        (
            RecordedRequest::Git {
                explicit_alias,
                repository,
                revision,
                selection,
            },
            DependencySourceRequest::Git {
                explicit_alias: alias,
                repository: actual_repository,
                revision: actual_revision,
                selection: actual_selection,
            },
        ) => {
            explicit_alias == alias
                && repository == actual_repository
                && revision == actual_revision
                && selection == actual_selection
        }
        _ => false,
    }
}
