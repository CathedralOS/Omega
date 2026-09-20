use super::model::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestionError,
    CanonicalPackageReconstructionQuestionLimits,
};
use crate::declarations::PackageKey;
use crate::lock::PackageOccurrenceRoster;
use crate::resolution::graph::{CanonicalDependencySourceSelection, CanonicalSourceClosureSubject};
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub(super) fn validate_association(
    source_closure: &CanonicalSourceClosureSubject,
    entries: &[CanonicalPackageReconstructionEntry],
    limits: CanonicalPackageReconstructionQuestionLimits,
) -> Result<(), CanonicalPackageReconstructionQuestionError> {
    if source_closure.packages().is_empty() {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "package reconstruction source closure is empty",
        ));
    }
    if source_closure.packages().len() > limits.maximum_packages {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "package reconstruction question exceeds its package-count ceiling",
        ));
    }
    let roster = PackageOccurrenceRoster::derive(source_closure).map_err(|_| {
        CanonicalPackageReconstructionQuestionError::new(
            "could not derive the source closure occurrence roster",
        )
    })?;
    if entries.len() != roster.occurrence_count() {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "source closure and obligation ledger occurrences are not bijective",
        ));
    }

    let mut identities = BTreeMap::<PackageKeyIdentity, &PackageKey>::new();
    for source in source_closure.packages() {
        if identities
            .insert(source.key().identity(), source.key())
            .is_some()
        {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "distinct package keys collide on compiler package identity",
            ));
        }
    }

    let execution_profile = entries
        .first()
        .and_then(|entry| entry.context().build_execution_profile());
    let outgoing = outgoing_product_requests(source_closure)?;
    let expected = roster.coverages().iter().flat_map(|coverage| {
        coverage
            .purposes()
            .iter()
            .map(move |purpose| (coverage.package(), *purpose))
    });
    for ((package, purpose), entry) in expected.zip(entries) {
        if entry.package() != package || entry.context().purpose() != purpose {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction entries do not match canonical source-package/purpose order",
            ));
        }
        let context = entry.context();
        let expected_target = if purpose.is_product() {
            Some(source_closure.target_profile())
        } else {
            execution_profile
        };
        if context.build_execution_profile() != execution_profile
            || Some(context.target()) != expected_target
            || entry.obligations.target() != context.target()
        {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction entry has a mismatched checked target or execution profile",
            ));
        }
        if entry.obligations.package() != package.identity() {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "obligation ledger root identity does not match its source package",
            ));
        }
        // Ordinary dependencies inherit this occurrence's context. Its own
        // build-only edges belong to separately reconstructed activations.
        validate_ledger_source_closure(source_closure, &outgoing, entry)?;
    }
    Ok(())
}

fn validate_ledger_source_closure(
    source_closure: &CanonicalSourceClosureSubject,
    outgoing: &[&[CanonicalDependencySourceSelection]],
    entry: &CanonicalPackageReconstructionEntry,
) -> Result<(), CanonicalPackageReconstructionQuestionError> {
    let reachable = reachable_source_packages(source_closure, outgoing, &entry.package)?;
    let mut expected_packages = reachable
        .iter()
        .map(|&package_index| source_closure.packages()[package_index].key().identity())
        .collect::<Vec<_>>();
    expected_packages.sort_unstable();
    if entry.obligations.dependency_closure().packages() != expected_packages {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "obligation ledger package closure does not match the source subject",
        ));
    }

    let mut expected_dependencies = reachable
        .iter()
        .flat_map(|&package_index| outgoing[package_index])
        .map(|dependency| {
            (
                dependency.requester().identity(),
                dependency.alias().as_str(),
                dependency.selected().key().identity(),
            )
        })
        .collect::<Vec<_>>();
    expected_dependencies.sort_unstable();
    let actual_dependencies = entry
        .obligations
        .dependency_closure()
        .dependencies()
        .iter()
        .map(|dependency| {
            (
                dependency.requester(),
                dependency.alias(),
                dependency.target(),
            )
        });
    if !expected_dependencies.into_iter().eq(actual_dependencies) {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "obligation ledger dependency edges do not match the source subject",
        ));
    }
    Ok(())
}

// The validated subject orders requests by exact requester, purpose, and
// declaration index. Borrow its product groups once for every ledger below.
fn outgoing_product_requests(
    source_closure: &CanonicalSourceClosureSubject,
) -> Result<Vec<&[CanonicalDependencySourceSelection]>, CanonicalPackageReconstructionQuestionError>
{
    let mut outgoing = Vec::new();
    outgoing
        .try_reserve_exact(source_closure.packages().len())
        .map_err(|_| {
            CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction graph allocation failed",
            )
        })?;
    let mut remaining = source_closure.dependency_requests();
    for package in source_closure.packages() {
        let request_count =
            remaining.partition_point(|request| request.requester() == package.key());
        let (requests, rest) = remaining.split_at(request_count);
        let product_count = requests.partition_point(|request| request.purpose().is_product());
        outgoing.push(&requests[..product_count]);
        remaining = rest;
    }
    Ok(outgoing)
}

fn reachable_source_packages(
    source_closure: &CanonicalSourceClosureSubject,
    outgoing: &[&[CanonicalDependencySourceSelection]],
    root: &PackageKey,
) -> Result<Vec<usize>, CanonicalPackageReconstructionQuestionError> {
    let packages = source_closure.packages();
    let package_position = |key: &PackageKey| {
        packages
            .binary_search_by(|package| package.key().cmp(key))
            .map_err(|_| {
                CanonicalPackageReconstructionQuestionError::new(
                    "package reconstruction graph contains an unknown package",
                )
            })
    };
    let mut reachable = Vec::new();
    let mut visited = Vec::new();
    reachable.try_reserve_exact(packages.len()).map_err(|_| {
        CanonicalPackageReconstructionQuestionError::new(
            "package reconstruction graph allocation failed",
        )
    })?;
    visited.try_reserve_exact(packages.len()).map_err(|_| {
        CanonicalPackageReconstructionQuestionError::new(
            "package reconstruction graph allocation failed",
        )
    })?;
    visited.resize(packages.len(), false);
    let root_position = package_position(root)?;
    visited[root_position] = true;
    reachable.push(root_position);
    let mut next_package = 0;
    while next_package < reachable.len() {
        for dependency in outgoing[reachable[next_package]] {
            let selected = package_position(dependency.selected().key())?;
            if !visited[selected] {
                visited[selected] = true;
                reachable.push(selected);
            }
        }
        next_package += 1;
    }
    Ok(reachable)
}
