//! Pure joins performed before any dependent acquisition.

use super::ResolveLockedPackageClosureError as Error;
use crate::declarations::dependencies::read::DependencySourceRequest;
use crate::resolution::graph::{
    CanonicalDependencySourceRequest as RecordedRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    PackageRootSourceRequest, PackageSourceClosureLimits, ResolvedSourceIdentity,
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
    if actual.key() != expected.key() || actual.resolution() != expected.resolution() {
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
) -> Result<&'a CanonicalDependencySourceSelection, Error> {
    let package_index = subject
        .packages()
        .binary_search_by(|source| source.key().cmp(requester.key()))
        .map_err(|_| Error::mismatch(requester.key(), "requester is absent from locked graph"))?;
    custody(subject, &subject.packages()[package_index], requester)?;
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
