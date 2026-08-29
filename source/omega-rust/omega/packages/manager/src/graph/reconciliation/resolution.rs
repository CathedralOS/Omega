//! Bounded traversal, custody reconciliation, and conflict path collection.

use super::super::validation::{ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode};
use super::model::{
    DependencyRequestPath, DependencyRequestPathStep, PackageSourceClosureConflict,
    PackageSourceClosureConflictCandidate, PackageSourceClosureLimitKind,
    PackageSourceClosureLimits, PackageSourceClosureResolutionError,
};
use super::resolved_closure::ResolvedPackageSourceClosure;
use super::source_custody::{PackageRootSourceRequest, PackageSourceCustody};
use crate::manifest::dependency_projection::DependencySourceRequest;
use crate::source::identity::{AliasName, PackageKey};
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
        PackageSourceClosureLimits::default(),
        resolve_dependency,
    )
}

/// Resolve a complete source closure under caller-selected ceilings no looser
/// than the authority the caller is prepared to spend on hostile graph input.
pub(crate) fn resolve_package_source_closure_with_limits<E, F>(
    root_request: PackageRootSourceRequest,
    root: PackageSourceCustody,
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
        let mut resolved_dependencies = Vec::with_capacity(requester.dependency_requests().len());

        for (dependency_index, request) in requester.dependency_requests().iter().enumerate() {
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
            let alias = request.resolved_alias(dependency.key().name());
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
                .find(|candidate| candidate.custody == dependency)
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

    let conflicts = collect_conflicts(&root_key, &observed, &dependencies);
    if !conflicts.is_empty() {
        return Err(PackageSourceClosureResolutionError::ConflictingCustody { conflicts });
    }

    let nodes = accepted
        .values()
        .map(|custody| {
            ResolvedPackageNode::new(
                custody.source_identity(),
                dependencies.get(custody.key()).cloned().unwrap_or_default(),
            )
        })
        .collect();
    let graph = ResolvedPackageClosure::new(root_key, nodes)
        .map_err(|errors| PackageSourceClosureResolutionError::InvalidClosure { errors })?;

    let custodies: Vec<_> = accepted.into_values().collect();
    let custody_indices = custodies
        .iter()
        .enumerate()
        .map(|(index, custody)| (custody.key().clone(), index))
        .collect();

    Ok(ResolvedPackageSourceClosure {
        root_request,
        graph,
        custodies,
        custody_indices,
    })
}

fn collect_conflicts(
    root: &PackageKey,
    observed: &BTreeMap<PackageKey, Vec<ObservedCustody>>,
    dependencies: &BTreeMap<PackageKey, Vec<ResolvedDependency>>,
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
                        .flat_map(|origin| paths_for_origin(root, origin, key, dependencies))
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
            let mut requester_paths = paths_to_package(root, requester, dependencies);
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
) -> Vec<DependencyRequestPath> {
    let mut paths = Vec::new();
    let mut steps = Vec::new();
    let mut active = BTreeSet::new();
    collect_paths(
        root,
        root,
        target,
        dependencies,
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
        for (dependency_index, dependency) in outgoing.iter().enumerate() {
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
                active,
                steps,
                paths,
            );
            steps.pop();
        }
    }

    active.remove(current);
}
