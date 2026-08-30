//! Bounded traversal, custody reconciliation, and conflict path collection.

use super::super::{
    ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode, ResolvedSourceIdentity,
};
use super::model::{
    DependencyRequestPath, DependencyRequestPathStep, PackageSourceClosureConflict,
    PackageSourceClosureConflictCandidate, PackageSourceClosureLimitKind,
    PackageSourceClosureLimits, PackageSourceClosureResolutionError,
};
use super::resolved_closure::ResolvedPackageSourceClosure;
use crate::declarations::BuildDeclarationKind;
use crate::declarations::dependencies::read::DependencySourceRequest;
use crate::declarations::{AliasName, PackageKey};
use crate::resolution::graph::PackageRootSourceRequest;
use crate::resolution::source::PackageSourceCustody;
use omega_target::TargetProfile;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone)]
enum CustodyOrigin {
    Root,
    Dependency {
        requester: PackageKey,
        dependency_index: usize,
        alias: AliasName,
    },
}

#[derive(Debug)]
struct ObservedCustody {
    custody: PackageSourceCustody,
    origins: Vec<CustodyOrigin>,
}

/// Resolve every projected source request before returning a package graph.
///
/// Transport and requester-relative path interpretation remain entirely in
/// `resolve_dependency`. The callback receives the requester's exact custody
/// and one projected request and must return custody derived from a concrete
/// `ResolvedPackageSource<S>` adapter result.
#[cfg(test)]
pub(crate) fn resolve_package_source_closure<E, F>(
    root_request: PackageRootSourceRequest,
    root: PackageSourceCustody,
    resolve_dependency: F,
) -> Result<ResolvedPackageSourceClosure, PackageSourceClosureResolutionError<E>>
where
    F: FnMut(&PackageSourceCustody, &DependencySourceRequest) -> Result<PackageSourceCustody, E>,
{
    resolve_package_source_closure_with_limits(
        root_request,
        root,
        TargetProfile::CrossPlatformCli,
        PackageSourceClosureLimits::default(),
        resolve_dependency,
    )
}

/// Resolve a complete source closure under caller-selected ceilings no looser
/// than the authority the caller is prepared to spend on hostile graph input.
pub(crate) fn resolve_package_source_closure_with_limits<E, F>(
    root_request: PackageRootSourceRequest,
    root: PackageSourceCustody,
    target_profile: TargetProfile,
    limits: PackageSourceClosureLimits,
    mut resolve_dependency: F,
) -> Result<ResolvedPackageSourceClosure, PackageSourceClosureResolutionError<E>>
where
    F: FnMut(&PackageSourceCustody, &DependencySourceRequest) -> Result<PackageSourceCustody, E>,
{
    if limits.max_packages == 0 {
        return Err(PackageSourceClosureResolutionError::LimitExceeded {
            kind: PackageSourceClosureLimitKind::Packages,
            limit: limits.max_packages,
        });
    }
    let root_key = root.key().clone();
    let root_role = root.role();
    let mut accepted = BTreeMap::<PackageKey, PackageSourceCustody>::new();
    accepted.insert(root_key.clone(), root.clone());

    let mut observed = BTreeMap::<PackageKey, Vec<ObservedCustody>>::new();
    observed.insert(
        root_key.clone(),
        vec![ObservedCustody {
            custody: root,
            origins: vec![CustodyOrigin::Root],
        }],
    );

    let mut dependencies = BTreeMap::<PackageKey, Vec<ResolvedDependency>>::new();
    let mut depths = BTreeMap::from([(root_key.clone(), 0usize)]);
    let mut dependency_request_count = 0usize;
    let mut pending = VecDeque::from([root_key.clone()]);

    while let Some(requester_key) = pending.pop_front() {
        let requester = accepted
            .get(&requester_key)
            .expect("only accepted package custody enters the traversal queue")
            .clone();
        let requester_depth = depths[&requester_key];
        let active_occurrences = requester
            .projected_dependencies()
            .occurrence_indices_for_profile(target_profile)
            .collect::<Vec<_>>();
        let mut selected_dependencies = Vec::with_capacity(active_occurrences.len());

        for dependency_index in active_occurrences {
            let request = &requester.dependency_requests()[dependency_index];
            dependency_request_count = dependency_request_count.saturating_add(1);
            if dependency_request_count > limits.max_dependency_requests {
                return Err(PackageSourceClosureResolutionError::LimitExceeded {
                    kind: PackageSourceClosureLimitKind::DependencyRequests,
                    limit: limits.max_dependency_requests,
                });
            }
            let dependency_depth = requester_depth.saturating_add(1);
            if dependency_depth > limits.max_depth {
                return Err(PackageSourceClosureResolutionError::LimitExceeded {
                    kind: PackageSourceClosureLimitKind::Depth,
                    limit: limits.max_depth,
                });
            }
            let dependency = resolve_dependency(&requester, request).map_err(|error| {
                PackageSourceClosureResolutionError::Adapter {
                    requester: requester_key.clone(),
                    dependency_index,
                    request: request.clone(),
                    error,
                }
            })?;
            if dependency.role() != BuildDeclarationKind::Package {
                return Err(PackageSourceClosureResolutionError::InvalidDependencyRole {
                    requester: requester_key.clone(),
                    dependency_index,
                    selected: dependency.key().clone(),
                    role: dependency.role(),
                });
            }
            let alias = request.resolved_alias(dependency.key().name());
            selected_dependencies.push((dependency_index, dependency, alias, dependency_depth));
        }

        let selected_package_names = selected_dependencies
            .iter()
            .map(|(_, dependency, _, _)| dependency.key().name().clone())
            .collect::<Vec<_>>();
        requester
            .projected_dependencies()
            .validate_active_aliases(target_profile, &selected_package_names)
            .map_err(
                |error| PackageSourceClosureResolutionError::InvalidActiveAliases {
                    requester: requester_key.clone(),
                    error,
                },
            )?;

        let mut resolved_dependencies = Vec::with_capacity(selected_dependencies.len());
        for (dependency_index, dependency, alias, dependency_depth) in selected_dependencies {
            let target = dependency.key().clone();
            resolved_dependencies.push(ResolvedDependency::new(alias.clone(), target.clone()));

            let origin = CustodyOrigin::Dependency {
                requester: requester_key.clone(),
                dependency_index,
                alias,
            };
            let candidates = observed.entry(target.clone()).or_default();
            if let Some(candidate) = candidates
                .iter_mut()
                .find(|candidate| candidate.custody.semantically_equivalent(&dependency))
            {
                candidate.origins.push(origin);
            } else {
                candidates.push(ObservedCustody {
                    custody: dependency.clone(),
                    origins: vec![origin],
                });
            }

            if !accepted.contains_key(&target) {
                if accepted.len() >= limits.max_packages {
                    return Err(PackageSourceClosureResolutionError::LimitExceeded {
                        kind: PackageSourceClosureLimitKind::Packages,
                        limit: limits.max_packages,
                    });
                }
                accepted.insert(target.clone(), dependency);
                depths.insert(target.clone(), dependency_depth);
                pending.push_back(target);
            }
        }

        dependencies.insert(requester_key, resolved_dependencies);
    }

    let conflicts = collect_conflicts(
        &root_key,
        &observed,
        &dependencies,
        &accepted,
        target_profile,
    );
    if !conflicts.is_empty() {
        return Err(PackageSourceClosureResolutionError::ConflictingCustody { conflicts });
    }

    let nodes = accepted
        .values()
        .map(|custody| {
            ResolvedPackageNode::new(
                ResolvedSourceIdentity::from_validated_parts(
                    custody.key().clone(),
                    custody.resolution().clone(),
                ),
                dependencies.get(custody.key()).cloned().unwrap_or_default(),
            )
        })
        .collect();
    let graph = ResolvedPackageClosure::new(root_key, root_role, nodes)
        .map_err(|errors| PackageSourceClosureResolutionError::InvalidClosure { errors })?;

    let custodies: Vec<_> = accepted.into_values().collect();
    let custody_indices = custodies
        .iter()
        .enumerate()
        .map(|(index, custody)| (custody.key().clone(), index))
        .collect();

    Ok(ResolvedPackageSourceClosure {
        root_request,
        target_profile,
        graph,
        custodies,
        custody_indices,
    })
}

fn collect_conflicts(
    root: &PackageKey,
    observed: &BTreeMap<PackageKey, Vec<ObservedCustody>>,
    dependencies: &BTreeMap<PackageKey, Vec<ResolvedDependency>>,
    custodies: &BTreeMap<PackageKey, PackageSourceCustody>,
    target_profile: TargetProfile,
) -> Vec<PackageSourceClosureConflict> {
    observed
        .iter()
        .filter(|(_, candidates)| candidates.len() > 1)
        .map(|(key, candidates)| PackageSourceClosureConflict {
            key: key.clone(),
            candidates: candidates
                .iter()
                .map(|candidate| PackageSourceClosureConflictCandidate {
                    custody: candidate.custody.clone(),
                    requesting_paths: candidate
                        .origins
                        .iter()
                        .flat_map(|origin| {
                            paths_for_origin(
                                root,
                                origin,
                                key,
                                dependencies,
                                custodies,
                                target_profile,
                            )
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

fn paths_for_origin(
    root: &PackageKey,
    origin: &CustodyOrigin,
    target: &PackageKey,
    dependencies: &BTreeMap<PackageKey, Vec<ResolvedDependency>>,
    custodies: &BTreeMap<PackageKey, PackageSourceCustody>,
    target_profile: TargetProfile,
) -> Vec<DependencyRequestPath> {
    match origin {
        CustodyOrigin::Root => vec![DependencyRequestPath {
            root: root.clone(),
            steps: Vec::new(),
        }],
        CustodyOrigin::Dependency {
            requester,
            dependency_index,
            alias,
        } => {
            let mut requester_paths =
                paths_to_package(root, requester, dependencies, custodies, target_profile);
            for path in &mut requester_paths {
                path.steps.push(DependencyRequestPathStep {
                    requester: requester.clone(),
                    dependency_index: *dependency_index,
                    alias: alias.clone(),
                    target: target.clone(),
                });
            }
            requester_paths
        }
    }
}

fn paths_to_package(
    root: &PackageKey,
    target: &PackageKey,
    dependencies: &BTreeMap<PackageKey, Vec<ResolvedDependency>>,
    custodies: &BTreeMap<PackageKey, PackageSourceCustody>,
    target_profile: TargetProfile,
) -> Vec<DependencyRequestPath> {
    let mut paths = Vec::new();
    let mut steps = Vec::new();
    let mut active = BTreeSet::new();
    collect_paths(
        root,
        root,
        target,
        dependencies,
        custodies,
        target_profile,
        &mut active,
        &mut steps,
        &mut paths,
    );
    paths
}

#[allow(clippy::too_many_arguments)]
fn collect_paths(
    root: &PackageKey,
    current: &PackageKey,
    target: &PackageKey,
    dependencies: &BTreeMap<PackageKey, Vec<ResolvedDependency>>,
    custodies: &BTreeMap<PackageKey, PackageSourceCustody>,
    target_profile: TargetProfile,
    active: &mut BTreeSet<PackageKey>,
    steps: &mut Vec<DependencyRequestPathStep>,
    paths: &mut Vec<DependencyRequestPath>,
) {
    if current == target {
        paths.push(DependencyRequestPath {
            root: root.clone(),
            steps: steps.clone(),
        });
        return;
    }
    if !active.insert(current.clone()) {
        return;
    }

    if let Some(outgoing) = dependencies.get(current) {
        let active_occurrences = custodies[current]
            .projected_dependencies()
            .occurrence_indices_for_profile(target_profile)
            .collect::<Vec<_>>();
        for (active_index, dependency) in outgoing.iter().enumerate() {
            let dependency_index = active_occurrences[active_index];
            if active.contains(dependency.target()) {
                continue;
            }
            steps.push(DependencyRequestPathStep {
                requester: current.clone(),
                dependency_index,
                alias: dependency.alias().clone(),
                target: dependency.target().clone(),
            });
            collect_paths(
                root,
                dependency.target(),
                target,
                dependencies,
                custodies,
                target_profile,
                active,
                steps,
                paths,
            );
            steps.pop();
        }
    }

    active.remove(current);
}
