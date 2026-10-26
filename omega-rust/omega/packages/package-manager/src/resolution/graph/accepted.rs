//! Accepted-source comparisons over executed graphs, without source acquisition.

use crate::declarations::PackageKey;
use crate::declarations::dependencies::read::{DependencyPurpose, DependencySourceRequest};
use crate::resolution::graph::{
    CanonicalDependencySourceRequest as RecordedRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectError,
    CanonicalSourceClosureSubjectLimits, PackageRootSourceRequest, ResolvedPackageSourceClosure,
    ResolvedSourceIdentity,
};
use crate::resolution::source::PackageSourceCustody;

/// Refuse a changed edge before fetching or opening its selected source.
pub(crate) fn admit_locked_source_request(
    subject: &CanonicalSourceClosureSubject,
    pending: &super::reconcile::PendingSourceResolution,
) -> Result<(), AcceptedSourceError> {
    edge(
        subject,
        pending.requester(),
        pending.purpose(),
        pending.dependency_index(),
        pending.source(),
    )?;
    Ok(())
}

fn checked_custody(
    subject: &CanonicalSourceClosureSubject,
    expected: &ResolvedSourceIdentity,
    actual: &PackageSourceCustody,
    mutable_root: bool,
) -> Result<(), AcceptedSourceError> {
    if actual.key() != expected.key()
        || (!mutable_root && actual.resolution() != expected.resolution())
    {
        return Err(AcceptedSourceError::mismatch(
            expected.key(),
            "fresh source key or immutable content differs",
        ));
    }
    if subject.package_navigation(expected.key()) != Some(actual.navigation()) {
        return Err(AcceptedSourceError::mismatch(
            expected.key(),
            "fresh source navigation differs",
        ));
    }
    if subject.package_dependency_projection(expected.key())
        != Some(actual.dependency_projections())
    {
        return Err(AcceptedSourceError::mismatch(
            expected.key(),
            "complete fresh dependency projection differs",
        ));
    }
    Ok(())
}

fn edge<'a>(
    subject: &'a CanonicalSourceClosureSubject,
    requester: &PackageSourceCustody,
    purpose: DependencyPurpose,
    ordinal: usize,
    request: &DependencySourceRequest,
) -> Result<&'a CanonicalDependencySourceSelection, AcceptedSourceError> {
    let package_index = subject
        .packages()
        .binary_search_by(|source| source.key().cmp(requester.key()))
        .map_err(|_| {
            AcceptedSourceError::mismatch(requester.key(), "requester is absent from locked graph")
        })?;
    checked_custody(
        subject,
        &subject.packages()[package_index],
        requester,
        requester.key() == subject.root().selected().key(),
    )?;
    let index = subject
        .dependency_requests()
        .binary_search_by(|edge| {
            edge.requester()
                .cmp(requester.key())
                .then(edge.purpose().cmp(&purpose))
                .then(edge.dependency_index().cmp(&ordinal))
        })
        .map_err(|_| {
            AcceptedSourceError::mismatch(
                requester.key(),
                "authored request occurrence is absent from locked graph",
            )
        })?;
    let edge = &subject.dependency_requests()[index];
    if !request_matches(edge.request(), request) {
        return Err(AcceptedSourceError::mismatch(
            requester.key(),
            "exact authored dependency request differs",
        ));
    }
    Ok(edge)
}

fn local_root_context_matches(
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
pub(crate) fn mutable_local_graph_matches(
    recorded: &CanonicalSourceClosureSubject,
    closure: &ResolvedPackageSourceClosure,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<bool, AcceptedSourceError> {
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

#[derive(Debug)]
pub(crate) enum AcceptedSourceError {
    SourceMismatch {
        package: PackageKey,
        detail: &'static str,
    },
    Subject(CanonicalSourceClosureSubjectError),
}

impl AcceptedSourceError {
    fn mismatch(package: &PackageKey, detail: &'static str) -> Self {
        Self::SourceMismatch {
            package: package.clone(),
            detail,
        }
    }
}

impl From<CanonicalSourceClosureSubjectError> for AcceptedSourceError {
    fn from(error: CanonicalSourceClosureSubjectError) -> Self {
        Self::Subject(error)
    }
}

impl std::fmt::Display for AcceptedSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceMismatch { package, detail } => {
                write!(formatter, "locked source {package:?}: {detail}")
            }
            Self::Subject(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AcceptedSourceError {}
