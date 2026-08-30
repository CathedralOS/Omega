//! Validation that the retained source rows form one canonical closed graph.

use super::super::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceSelection, CanonicalSourceClosureSubjectError,
    CanonicalSourceClosureSubjectLimits,
};
use super::dependency::{validate_dependency_request, validate_dependency_selection_kind};
use super::projection::validate_dependency_projection;
use super::root::validate_root_request;
use super::source::{validate_package_navigation, validate_source_identity};
use crate::resolution::graph::{
    ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode, ResolvedSourceIdentity,
};
use crate::manifest::PackageKey;
use crate::manifest::dependencies::read::ProjectedDependencies;
use crate::resolution::source::PackageSourceNavigation;
use omega_target::TargetProfile;
use std::collections::BTreeMap;

pub(in super::super) fn validate_subject(
    target_profile: TargetProfile,
    root: &CanonicalRootSourceSelection,
    packages: &[ResolvedSourceIdentity],
    package_navigations: &[PackageSourceNavigation],
    package_dependency_projections: &[ProjectedDependencies],
    dependency_requests: &[CanonicalDependencySourceSelection],
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if packages.is_empty()
        || packages.len() > limits.maximum_packages
        || packages.len() != package_navigations.len()
        || packages.len() != package_dependency_projections.len()
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
    let maximum_memberships = limits
        .maximum_dependency_requests
        .checked_mul(TargetProfile::ALL.len())
        .ok_or_else(|| {
            CanonicalSourceClosureSubjectError::new(
                "source-closure dependency membership limit overflowed",
            )
        })?;
    let mut occurrence_count = 0usize;
    let mut membership_count = 0usize;
    for projection in package_dependency_projections {
        let (projection_occurrences, projection_memberships) =
            validate_dependency_projection(projection, limits)?;
        occurrence_count = occurrence_count
            .checked_add(projection_occurrences)
            .filter(|count| *count <= limits.maximum_dependency_requests)
            .ok_or_else(|| {
                CanonicalSourceClosureSubjectError::new(
                    "source-closure dependency projections exceed their request-count limit",
                )
            })?;
        membership_count = membership_count
            .checked_add(projection_memberships)
            .filter(|count| *count <= maximum_memberships)
            .ok_or_else(|| {
                CanonicalSourceClosureSubjectError::new(
                    "source-closure dependency projections exceed their membership limit",
                )
            })?;
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
    let projection_by_key = packages
        .iter()
        .zip(package_dependency_projections)
        .map(|(source, projection)| (source.key(), projection))
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
    let mut selected_occurrences = BTreeMap::<PackageKey, Vec<usize>>::new();
    for selection in dependency_requests {
        validate_source_identity(&selection.selected, limits.maximum_identity_bytes)?;
        validate_dependency_request(&selection.request, limits.maximum_request_bytes)?;
        if package_by_key.get(&selection.requester).is_none() {
            return Err(CanonicalSourceClosureSubjectError::new(
                "dependency request names an unknown requester",
            ));
        }
        let projection = projection_by_key[&selection.requester];
        let Some(projected_request) = projection
            .authored_dependencies()
            .get(selection.dependency_index)
        else {
            return Err(CanonicalSourceClosureSubjectError::new(
                "active dependency selection names an unknown authored occurrence",
            ));
        };
        if CanonicalDependencySourceRequest::from(projected_request) != selection.request {
            return Err(CanonicalSourceClosureSubjectError::new(
                "active dependency selection disagrees with its complete projection",
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
                if selection.dependency_index <= previous_index {
                    return Err(CanonicalSourceClosureSubjectError::new(
                        "dependency requests are not in strict canonical order",
                    ));
                }
            }
            Some((requester, _)) if requester >= &selection.requester => {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "dependency requests are not in strict canonical order",
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
        selected_occurrences
            .entry(selection.requester.clone())
            .or_default()
            .push(selection.dependency_index);
        previous = Some((&selection.requester, selection.dependency_index));
    }

    for source in packages {
        let expected = projection_by_key[source.key()]
            .occurrence_indices_for_profile(target_profile)
            .collect::<Vec<_>>();
        let actual = selected_occurrences
            .get(source.key())
            .map(Vec::as_slice)
            .unwrap_or_default();
        if actual != expected {
            return Err(CanonicalSourceClosureSubjectError::new(
                "active dependency selections do not match the selected target profile",
            ));
        }
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
    ResolvedPackageClosure::new(root.selected.key().clone(), root.role(), nodes).map_err(|_| {
        CanonicalSourceClosureSubjectError::new(
            "source-closure subject does not form one closed reachable acyclic graph",
        )
    })?;
    Ok(())
}
