//! Validate the complete canonical graph using borrowed rows and bounded scratch.

use super::super::{
    CanonicalDependencySourceSelection, CanonicalRootSourceSelection,
    CanonicalSourceClosureSubjectError as Error, CanonicalSourceClosureSubjectLimits as Limits,
    request_view::Request, usage::Budget,
};
use super::dependency::{validate_dependency_request, validate_dependency_selection_kind};
use super::root::validate_root_request;
use super::source::{validate_package_navigation, validate_source_identity};
use crate::declarations::dependencies::read::ProjectedDependencies;
use crate::resolution::graph::ResolvedSourceIdentity;
use crate::resolution::source::PackageSourceNavigation;

pub(in super::super) fn validate_subject(
    root: &CanonicalRootSourceSelection,
    packages: &[ResolvedSourceIdentity],
    navigations: &[PackageSourceNavigation],
    projections: &[ProjectedDependencies],
    edges: &[CanonicalDependencySourceSelection],
    limits: Limits,
) -> Result<(), Error> {
    validate_subject_with_budget(
        root,
        packages,
        navigations,
        projections,
        edges,
        limits,
        &mut Budget::new(usize::MAX),
    )
}

pub(in super::super) fn validate_subject_with_budget(
    root: &CanonicalRootSourceSelection,
    packages: &[ResolvedSourceIdentity],
    navigations: &[PackageSourceNavigation],
    projections: &[ProjectedDependencies],
    edges: &[CanonicalDependencySourceSelection],
    limits: Limits,
    budget: &mut Budget,
) -> Result<(), Error> {
    if packages.is_empty()
        || packages.len() > limits.maximum_packages
        || packages.len() != navigations.len()
        || packages.len() != projections.len()
    {
        return Err(Error::new(
            "source-closure subject violates its package-count limit",
        ));
    }
    if edges.len() > limits.maximum_dependency_requests {
        return Err(Error::new(
            "source-closure subject violates its request-count limit",
        ));
    }
    for source in packages {
        validate_source_identity(source, limits.maximum_identity_bytes)?;
    }
    let mut authored = 0usize;
    for projection in projections {
        authored = authored
            .checked_add(projection.authored_dependencies().len())
            .filter(|count| *count <= limits.maximum_dependency_requests)
            .ok_or_else(|| {
                Error::new("source-closure dependency projections exceed their request-count limit")
            })?;
        for request in projection.authored_dependencies() {
            validate_dependency_request(Request::from(request), limits.maximum_request_bytes)?;
        }
    }
    if authored != edges.len() {
        return Err(Error::new(
            "source-closure authored and selected dependency counts disagree",
        ));
    }
    if packages
        .windows(2)
        .any(|pair| pair[0].key() >= pair[1].key())
    {
        return Err(Error::new(
            "source-closure packages are not in strict canonical order",
        ));
    }
    validate_source_identity(&root.selected, limits.maximum_identity_bytes)?;
    let root_index = packages
        .binary_search_by(|source| source.key().cmp(root.selected.key()))
        .ok()
        .filter(|index| packages[*index] == root.selected)
        .ok_or_else(|| Error::new("root request selection is absent or resolution-mismatched"))?;
    for (source, navigation) in packages.iter().zip(navigations) {
        validate_package_navigation(source, navigation)?;
    }
    validate_root_request(root, &navigations[root_index], limits, budget)?;
    let mut previous: Option<&CanonicalDependencySourceSelection> = None;
    for edge in edges {
        validate_source_identity(&edge.selected, limits.maximum_identity_bytes)?;
        validate_dependency_request(Request::from(&edge.request), limits.maximum_request_bytes)?;
        let requester = packages
            .binary_search_by(|source| source.key().cmp(&edge.requester))
            .map_err(|_| Error::new("dependency request names an unknown requester"))?;
        let projected = projections[requester]
            .authored_dependencies()
            .get(edge.dependency_index)
            .ok_or_else(|| {
                Error::new("active dependency selection names an unknown authored occurrence")
            })?;
        if Request::from(projected) != Request::from(&edge.request) {
            return Err(Error::new(
                "active dependency selection disagrees with its complete projection",
            ));
        }
        let selected = packages
            .binary_search_by(|source| source.key().cmp(edge.selected.key()))
            .ok()
            .filter(|index| packages[*index] == edge.selected)
            .ok_or_else(|| {
                Error::new("dependency request selection is absent or resolution-mismatched")
            })?;
        let alias_matches = match edge.request.explicit_alias() {
            Some(alias) => alias == &edge.alias,
            None => edge.alias.as_str().bytes().eq(edge
                .selected
                .key()
                .name()
                .as_str()
                .bytes()
                .map(|byte| if byte == b'-' { b'_' } else { byte })),
        };
        if !alias_matches {
            return Err(Error::new(
                "dependency request alias disagrees with its authored selection",
            ));
        }
        if previous.is_some_and(|previous| {
            previous.requester > edge.requester
                || (previous.requester == edge.requester
                    && previous.dependency_index >= edge.dependency_index)
        }) {
            return Err(Error::new(
                "dependency requests are not in strict canonical order",
            ));
        }
        validate_dependency_selection_kind(
            edge,
            &packages[requester],
            &navigations[requester],
            &navigations[selected],
            budget,
        )?;
        previous = Some(edge);
    }
    let mut aliases = budget.reserve::<&str>(edges.len())?;
    for (source, projection) in packages.iter().zip(projections) {
        let selected = selected_edges(edges, source);
        if selected.len() != projection.authored_dependencies().len()
            || selected
                .iter()
                .enumerate()
                .any(|(index, edge)| edge.dependency_index != index)
        {
            return Err(Error::new(
                "dependency selections do not match the authored dependency list",
            ));
        }
        aliases.clear();
        aliases.extend(selected.iter().map(|edge| edge.alias.as_str()));
        aliases.sort_unstable();
        if aliases.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::new(
                "dependency aliases are not unique within their requester",
            ));
        }
    }
    super::walk::validate(packages, edges, root_index, budget)
}

pub(super) fn selected_edges<'a>(
    edges: &'a [CanonicalDependencySourceSelection],
    source: &ResolvedSourceIdentity,
) -> &'a [CanonicalDependencySourceSelection] {
    let start = edges.partition_point(|edge| &edge.requester < source.key());
    let end = edges[start..].partition_point(|edge| &edge.requester == source.key());
    &edges[start..start + end]
}
