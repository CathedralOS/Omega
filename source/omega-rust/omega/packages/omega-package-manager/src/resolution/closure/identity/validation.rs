use super::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubjectError,
    CanonicalSourceClosureSubjectLimits,
};
use crate::resolution::binding::PackageSourceNavigation;
use crate::resolution::closure::reconciliation::PackageRootSourceRequest;
use crate::resolution::closure::{
    ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode, ResolvedSourceIdentity,
};
use omega_package_source::GitSourceRequest;
use omega_package_source::{
    AliasName, ImmutableSourceResolution, PackageKey, SourceLineage, WorkspaceLineageIdentity,
};
use std::collections::BTreeMap;

pub(super) fn canonical_root_request(
    request: &PackageRootSourceRequest,
) -> CanonicalRootSourceRequest {
    match request {
        PackageRootSourceRequest::Git(request) => CanonicalRootSourceRequest::Git {
            requested_locator: request.acquisition().requested_locator().to_owned(),
            requested_revision: request.acquisition().requested_revision().to_owned(),
            selection: request.selection().clone(),
        },
        PackageRootSourceRequest::WorkspaceMember {
            workspace_root_source,
            member_path,
            requested_workspace_root,
        } => CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source: workspace_root_source.clone(),
            member_path: member_path.clone(),
            requested_workspace_root: requested_workspace_root
                .as_os_str()
                .as_encoded_bytes()
                .to_vec(),
        },
        PackageRootSourceRequest::ExternalLocal {
            requested_root,
            source_context,
        } => CanonicalRootSourceRequest::ExternalLocal {
            requested_root: requested_root.as_os_str().as_encoded_bytes().to_vec(),
            source_context: source_context.clone(),
        },
    }
}

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

fn validate_root_request(
    root: &CanonicalRootSourceSelection,
    navigation: &PackageSourceNavigation,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match &root.request {
        CanonicalRootSourceRequest::Git {
            requested_locator,
            requested_revision,
            selection,
        } => {
            validate_request_bytes(requested_locator.as_bytes(), limits.maximum_request_bytes)?;
            validate_request_bytes(requested_revision.as_bytes(), limits.maximum_request_bytes)?;
            let request =
                GitSourceRequest::new(requested_locator.clone(), Some(requested_revision.clone()))
                    .map_err(|_| {
                        CanonicalSourceClosureSubjectError::new("invalid root Git request")
                    })?;
            if request.lineage() != root.selected.key().source_lineage()
                || !matches!(
                    root.selected.resolution(),
                    ImmutableSourceResolution::Git { .. }
                )
            {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "root Git request disagrees with its selected source",
                ));
            }
            validate_git_selection(selection, &root.selected, navigation)?;
        }
        CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source,
            member_path,
            requested_workspace_root,
        } => {
            validate_source_lineage(workspace_root_source, limits.maximum_identity_bytes)?;
            validate_request_bytes(
                member_path.as_str().as_bytes(),
                limits.maximum_request_bytes,
            )?;
            validate_request_bytes(requested_workspace_root, limits.maximum_request_bytes)?;
            let identity = WorkspaceLineageIdentity::from_root_source(workspace_root_source)
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new(
                        "invalid workspace root source in root request",
                    )
                })?;
            if !matches!(
                root.selected.key().source_lineage(),
                SourceLineage::Workspace(lineage)
                    if lineage.workspace_identity() == &identity
                        && lineage.member_path() == member_path
            ) || !matches!(
                root.selected.resolution(),
                ImmutableSourceResolution::Workspace { .. }
            ) {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "workspace root request disagrees with its selected source",
                ));
            }
            if navigation != &PackageSourceNavigation::Root {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "workspace root request has non-root package navigation",
                ));
            }
        }
        CanonicalRootSourceRequest::ExternalLocal {
            requested_root,
            source_context,
        } => {
            validate_request_bytes(requested_root, limits.maximum_request_bytes)?;
            if !matches!(
                root.selected.key().source_lineage(),
                SourceLineage::ExternalLocal(lineage)
                    if lineage.source_context() == source_context
            ) || !matches!(
                root.selected.resolution(),
                ImmutableSourceResolution::ExternalLocal { .. }
            ) {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "external-local root request disagrees with its selected source",
                ));
            }
            if navigation != &PackageSourceNavigation::Root {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "external-local root request has non-root package navigation",
                ));
            }
        }
    }
    Ok(())
}

fn validate_dependency_selection_kind(
    selection: &CanonicalDependencySourceSelection,
    requester: &ResolvedSourceIdentity,
    _requester_navigation: &PackageSourceNavigation,
    selected_navigation: &PackageSourceNavigation,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match &selection.request {
        CanonicalDependencySourceRequest::Path { .. } => {
            match selection.selected.key().source_lineage() {
                SourceLineage::Workspace(_) | SourceLineage::ExternalLocal(_) => {
                    if selected_navigation != &PackageSourceNavigation::Root {
                        return Err(CanonicalSourceClosureSubjectError::new(
                            "path request selected invalid package navigation",
                        ));
                    }
                }
                SourceLineage::GitHub(_) | SourceLineage::GitLab(_) | SourceLineage::Git(_) => {
                    if requester.key().source_lineage() != selection.selected.key().source_lineage()
                        || requester.resolution() != selection.selected.resolution()
                        || !matches!(selected_navigation, PackageSourceNavigation::Member(_))
                    {
                        return Err(CanonicalSourceClosureSubjectError::new(
                            "Git member path request escaped its repository resolution",
                        ));
                    }
                }
            }
        }
        CanonicalDependencySourceRequest::Git {
            repository,
            revision,
            selection: package_selection,
            ..
        } => {
            let request = GitSourceRequest::new(repository.clone(), Some(revision.clone()))
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new("invalid dependency Git request")
                })?;
            if request.lineage() != selection.selected.key().source_lineage()
                || !matches!(
                    selection.selected.resolution(),
                    ImmutableSourceResolution::Git { .. }
                )
            {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "dependency Git request disagrees with its selected source",
                ));
            }
            validate_git_selection(package_selection, &selection.selected, selected_navigation)?;
        }
    }
    Ok(())
}

fn validate_package_navigation(
    source: &ResolvedSourceIdentity,
    navigation: &PackageSourceNavigation,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if matches!(navigation, PackageSourceNavigation::Member(_))
        && (!matches!(
            source.key().source_lineage(),
            SourceLineage::GitHub(_) | SourceLineage::GitLab(_) | SourceLineage::Git(_)
        ) || !matches!(source.resolution(), ImmutableSourceResolution::Git { .. }))
    {
        return Err(CanonicalSourceClosureSubjectError::new(
            "member navigation requires an immutable Git package",
        ));
    }
    Ok(())
}

fn validate_git_selection(
    selection: &crate::manifest::PackageSelection,
    selected: &ResolvedSourceIdentity,
    navigation: &PackageSourceNavigation,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match (selection, navigation) {
        (crate::manifest::PackageSelection::Root, PackageSourceNavigation::Root) => Ok(()),
        (crate::manifest::PackageSelection::Named(package), PackageSourceNavigation::Member(_))
            if package == selected.key().name() =>
        {
            Ok(())
        }
        (crate::manifest::PackageSelection::Named(package), _)
            if package != selected.key().name() =>
        {
            Err(CanonicalSourceClosureSubjectError::new(
                "named Git package selection disagrees with its selected package",
            ))
        }
        _ => Err(CanonicalSourceClosureSubjectError::new(
            "Git package selection disagrees with package navigation",
        )),
    }
}

fn validate_dependency_request(
    request: &CanonicalDependencySourceRequest,
    maximum_request_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match request {
        CanonicalDependencySourceRequest::Path {
            explicit_alias,
            location,
        } => {
            validate_optional_alias(explicit_alias)?;
            validate_request_bytes(location.as_bytes(), maximum_request_bytes)
        }
        CanonicalDependencySourceRequest::Git {
            explicit_alias,
            repository,
            revision,
            selection: _,
        } => {
            validate_optional_alias(explicit_alias)?;
            validate_request_bytes(repository.as_bytes(), maximum_request_bytes)?;
            validate_request_bytes(revision.as_bytes(), maximum_request_bytes)
        }
    }
}

fn validate_optional_alias(
    alias: &Option<AliasName>,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if alias
        .as_ref()
        .is_some_and(|alias| alias.as_str().is_empty())
    {
        Err(CanonicalSourceClosureSubjectError::new(
            "dependency request contains an empty explicit alias",
        ))
    } else {
        Ok(())
    }
}

fn validate_request_bytes(
    bytes: &[u8],
    maximum_request_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if bytes.len() > maximum_request_bytes {
        Err(CanonicalSourceClosureSubjectError::new(
            "source request violates its byte limit",
        ))
    } else {
        Ok(())
    }
}

fn validate_source_identity(
    source: &ResolvedSourceIdentity,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    validate_package_key(source.key(), maximum_identity_bytes)?;
    if !source
        .resolution()
        .matches_lineage(source.key().source_lineage())
    {
        return Err(CanonicalSourceClosureSubjectError::new(
            "source resolution disagrees with package lineage",
        ));
    }
    Ok(())
}

pub(super) fn validate_package_key(
    key: &PackageKey,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    validate_identity_string(key.name().as_str(), maximum_identity_bytes)?;
    validate_source_lineage(key.source_lineage(), maximum_identity_bytes)
}

pub(super) fn validate_source_lineage(
    lineage: &SourceLineage,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    let check = |value: &str| validate_identity_string(value, maximum_identity_bytes);
    match lineage {
        SourceLineage::GitHub(lineage) => {
            check(lineage.owner())?;
            check(lineage.repository())
        }
        SourceLineage::GitLab(lineage) => check(lineage.repository_path()),
        SourceLineage::Git(lineage) => {
            if let Some(user) = lineage.user() {
                check(user)?;
            }
            check(lineage.host())?;
            check(lineage.repository_path())
        }
        SourceLineage::Workspace(lineage) => check(lineage.member_path().as_str()),
        SourceLineage::ExternalLocal(lineage) => {
            check(lineage.canonical_absolute_path().to_str().ok_or_else(|| {
                CanonicalSourceClosureSubjectError::new("external-local lineage path is not UTF-8")
            })?)
        }
    }
}

fn validate_identity_string(
    value: &str,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if value.is_empty() || value.len() > maximum_identity_bytes {
        Err(CanonicalSourceClosureSubjectError::new(
            "source identity violates its byte bounds",
        ))
    } else {
        Ok(())
    }
}
