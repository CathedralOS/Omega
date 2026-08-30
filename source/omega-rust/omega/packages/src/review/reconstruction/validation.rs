use super::model::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestionError,
    CanonicalPackageReconstructionQuestionLimits,
};
use crate::declarations::PackageKey;
use crate::resolution::graph::CanonicalSourceClosureSubject;
use omega_package_evidence::obligations::encode_ordinary_package_obligation_ledger;
use psi_core::PackageKeyIdentity;
use std::collections::{BTreeMap, BTreeSet};

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
    if entries.len() != source_closure.packages().len() {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "source closure and obligation ledgers are not bijective",
        ));
    }
    if entries.len() > limits.maximum_packages {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "package reconstruction question exceeds its package-count ceiling",
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

    let expected_target = entries[0].obligations.target();
    let mut total_ledger_bytes = 0usize;
    for (source, entry) in source_closure.packages().iter().zip(entries) {
        if entry.package != *source.key() {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction entries are not in canonical source-package order",
            ));
        }
        if entry.obligations.package() != entry.package.identity() {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "obligation ledger root identity does not match its source package",
            ));
        }
        if entry.obligations.target() != expected_target {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction question mixes deployment targets",
            ));
        }
        validate_ledger_source_closure(source_closure, entry)?;
        let encoded =
            encode_ordinary_package_obligation_ledger(&entry.obligations).map_err(|_| {
                CanonicalPackageReconstructionQuestionError::new(
                    "package reconstruction question contains an invalid obligation ledger",
                )
            })?;
        if encoded.len() > limits.maximum_ledger_bytes {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction obligation ledger exceeds its byte ceiling",
            ));
        }
        total_ledger_bytes = total_ledger_bytes
            .checked_add(encoded.len())
            .ok_or_else(|| {
                CanonicalPackageReconstructionQuestionError::new(
                    "package reconstruction ledger-byte accounting overflowed",
                )
            })?;
        if total_ledger_bytes > limits.maximum_total_ledger_bytes {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction question exceeds its total ledger-byte ceiling",
            ));
        }
    }
    Ok(())
}

fn validate_ledger_source_closure(
    source_closure: &CanonicalSourceClosureSubject,
    entry: &CanonicalPackageReconstructionEntry,
) -> Result<(), CanonicalPackageReconstructionQuestionError> {
    let reachable = reachable_source_packages(source_closure, &entry.package);
    let mut expected_packages = reachable
        .iter()
        .map(PackageKey::identity)
        .collect::<Vec<_>>();
    expected_packages.sort_unstable();
    if entry.obligations.dependency_closure().packages() != expected_packages {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "obligation ledger package closure does not match the source subject",
        ));
    }

    let mut expected_dependencies = source_closure
        .dependency_requests()
        .iter()
        .filter(|dependency| reachable.contains(dependency.requester()))
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

fn reachable_source_packages(
    source_closure: &CanonicalSourceClosureSubject,
    root: &PackageKey,
) -> BTreeSet<PackageKey> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![root.clone()];
    while let Some(package) = pending.pop() {
        if !reachable.insert(package.clone()) {
            continue;
        }
        pending.extend(
            source_closure
                .dependency_requests()
                .iter()
                .filter(|dependency| dependency.requester() == &package)
                .map(|dependency| dependency.selected().key().clone()),
        );
    }
    reachable
}
