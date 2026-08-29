//! Canonical closure validation, organized by the question being checked.

mod dependency;
mod root;
mod source;

pub(super) use root::canonical_root_request;
pub(super) use source::{validate_package_key, validate_source_lineage};

use super::{
    CanonicalDependencySourceSelection, CanonicalRootSourceSelection,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectLimits,
};
use crate::identity::PackageKey;
use crate::resolution::closure::{
    ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode, ResolvedSourceIdentity,
};
use crate::resolution::source::PackageSourceNavigation;
use dependency::{validate_dependency_request, validate_dependency_selection_kind};
use root::validate_root_request;
use source::{validate_package_navigation, validate_source_identity};
use std::collections::BTreeMap;

pub(super) fn validate_subject(
    root: &CanonicalRootSourceSelection,
    packages: &[ResolvedSourceIdentity],
    package_navigations: &[PackageSourceNavigation],
    dependency_requests: &[CanonicalDependencySourceSelection],
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if packages.is_empty()
        || packages.len() > limits.maximum_packages
        || packages.len() != package_navigations.len()
    {
        return Err(CanonicalSourceClosureSubjectError::new(
            "source-closure subject violates its package-count limit",
        ));
    }
    if dependency_requests.len() > limits.maximum_dependency_requests {
        return Err(CanonicalSourceClosureSubjectError::new(
            "source-closure subject violates its request-count limit",
        ));
    }
    for source in packages {
        validate_source_identity(source, limits.maximum_identity_bytes)?;
    }
    if packages
        .windows(2)
        .any(|pair| pair[0].key() >= pair[1].key())
    {
        return Err(CanonicalSourceClosureSubjectError::new(
            "source-closure packages are not in strict canonical order",
        ));
    }
    validate_source_identity(&root.selected, limits.maximum_identity_bytes)?;
    let package_by_key = packages
        .iter()
        .map(|source| (source.key(), source))
        .collect::<BTreeMap<_, _>>();
    let navigation_by_key = packages
        .iter()
        .zip(package_navigations)
        .map(|(source, navigation)| (source.key(), navigation))
        .collect::<BTreeMap<_, _>>();
    for (source, navigation) in packages.iter().zip(package_navigations) {
        validate_package_navigation(source, navigation)?;
    }
    if package_by_key.get(root.selected.key()).copied() != Some(&root.selected) {
        return Err(CanonicalSourceClosureSubjectError::new(
            "root request selection is absent or resolution-mismatched",
        ));
    }
    let root_navigation = navigation_by_key
        .get(root.selected.key())
        .copied()
        .ok_or_else(|| {
            CanonicalSourceClosureSubjectError::new("root package navigation is absent")
        })?;
    validate_root_request(root, root_navigation, limits)?;

    let mut previous: Option<(&PackageKey, usize)> = None;
    let mut dependencies = BTreeMap::<PackageKey, Vec<ResolvedDependency>>::new();
    for selection in dependency_requests {
        validate_source_identity(&selection.selected, limits.maximum_identity_bytes)?;
        validate_dependency_request(&selection.request, limits.maximum_request_bytes)?;
        if package_by_key.get(&selection.requester).is_none() {
            return Err(CanonicalSourceClosureSubjectError::new(
                "dependency request names an unknown requester",
            ));
        }
        if package_by_key.get(selection.selected.key()).copied() != Some(&selection.selected) {
            return Err(CanonicalSourceClosureSubjectError::new(
                "dependency request selection is absent or resolution-mismatched",
            ));
        }
        if selection.alias
            != selection
                .request
                .resolved_alias(selection.selected.key().name())
        {
            return Err(CanonicalSourceClosureSubjectError::new(
                "dependency request alias disagrees with its authored selection",
            ));
        }
        match previous {
            Some((requester, previous_index)) if requester == &selection.requester => {
                if selection.dependency_index != previous_index + 1 {
                    return Err(CanonicalSourceClosureSubjectError::new(
                        "dependency request ordinals are not contiguous",
                    ));
                }
            }
            Some((requester, _)) if requester >= &selection.requester => {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "dependency requests are not in strict canonical order",
                ));
            }
            _ if selection.dependency_index != 0 => {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "dependency request ordinals do not begin at zero",
                ));
            }
            _ => {}
        }
        let requester_navigation = navigation_by_key
            .get(&selection.requester)
            .copied()
            .expect("known requester has navigation");
        let selected_navigation = navigation_by_key
            .get(selection.selected.key())
            .copied()
            .expect("known selected package has navigation");
        validate_dependency_selection_kind(
            selection,
            package_by_key[&selection.requester],
            requester_navigation,
            selected_navigation,
        )?;
        dependencies
            .entry(selection.requester.clone())
            .or_default()
            .push(ResolvedDependency::new(
                selection.alias.clone(),
                selection.selected.key().clone(),
            ));
        previous = Some((&selection.requester, selection.dependency_index));
    }

    let nodes = packages
        .iter()
        .map(|source| {
            ResolvedPackageNode::new(
                source.clone(),
                dependencies.remove(source.key()).unwrap_or_default(),
            )
        })
        .collect();
    ResolvedPackageClosure::new(root.selected.key().clone(), nodes).map_err(|_| {
        CanonicalSourceClosureSubjectError::new(
            "source-closure subject does not form one closed reachable acyclic graph",
        )
    })?;
    Ok(())
}
