//! Bounded traversal, custody reconciliation, and conflict path collection.

use super::super::{
    ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode, ResolvedSourceIdentity,
};
use super::resolved_closure::ResolvedPackageSourceClosure;
use super::{
    DependencyRequestPath, DependencyRequestPathStep, PackageSourceClosureConflict,
    PackageSourceClosureConflictCandidate, PackageSourceClosureLimitKind,
    PackageSourceClosureLimits, PackageSourceClosureResolutionError,
};
use crate::declarations::BuildDeclarationKind;
use crate::declarations::dependencies::read::{DependencyPurpose, DependencySourceRequest};
use crate::declarations::{AliasName, PackageKey};
use crate::resolution::graph::PackageRootSourceRequest;
use crate::resolution::source::PackageSourceCustody;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
enum CustodyOrigin {
    Root,
    Dependency {
        requester: PackageKey,
        purpose: DependencyPurpose,
        dependency_index: usize,
        alias: AliasName,
    },
}

#[derive(Debug)]
struct ObservedCustody {
    custody: PackageSourceCustody,
    origins: Vec<CustodyOrigin>,
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
    resolve_package_source_closure_with_indexed_limits(
        root_request,
        root,
        limits,
        |requester, _, _, request| resolve_dependency(requester, request),
    )
}

/// The same custody traversal with the exact requester-owned authored
/// coordinate. Locked recovery uses `(purpose, index)` to join distinct
/// request occurrences.
pub(crate) fn resolve_package_source_closure_with_indexed_limits<E, F>(
    root_request: PackageRootSourceRequest,
    root: PackageSourceCustody,
    limits: PackageSourceClosureLimits,
    mut resolve_dependency: F,
) -> Result<ResolvedPackageSourceClosure, PackageSourceClosureResolutionError<E>>
where
    F: FnMut(
        &PackageSourceCustody,
        DependencyPurpose,
        usize,
        &DependencySourceRequest,
    ) -> Result<PackageSourceCustody, E>,
{
    let mut step = begin_source_closure(root_request, root, limits)?;
    loop {
        let request = match step {
            SourceClosureStep::Complete(closure) => return Ok(closure),
            SourceClosureStep::Request(request) => request,
        };
        let dependency = resolve_dependency(
            request.requester(),
            request.purpose(),
            request.dependency_index(),
            request.source(),
        );
        step = match dependency {
            Ok(dependency) => request.complete(dependency)?,
            Err(error) => return Err(request.fail(error)),
        };
    }
}

/// A request owns the traversal until its selected source has a declaration.
/// Acquisition can therefore yield to application-owned Build execution without
/// a compiler callback, a provisional key, or a second graph reconciliation.
pub(crate) enum SourceClosureStep {
    Request(PendingSourceResolution),
    Complete(ResolvedPackageSourceClosure),
}

struct SourceClosureTraversal {
    root_request: PackageRootSourceRequest,
    root_key: PackageKey,
    root_role: BuildDeclarationKind,
    limits: PackageSourceClosureLimits,
    accepted: BTreeMap<PackageKey, PackageSourceCustody>,
    // Acquisition identity exists before the Build-produced package name.
    // Executed-source adapters reuse their invocation's first capture; graph
    // reconciliation still validates every alias, role, edge and cycle.
    acquired: BTreeMap<package_source::SourceLineage, PackageSourceCustody>,
    observed: BTreeMap<PackageKey, Vec<ObservedCustody>>,
    dependencies: BTreeMap<PackageKey, Vec<ResolvedDependency>>,
    depths: BTreeMap<PackageKey, usize>,
    dependency_request_count: usize,
    pending: VecDeque<PackageKey>,
}

struct SelectedDependency {
    purpose: DependencyPurpose,
    dependency_index: usize,
    custody: PackageSourceCustody,
    alias: AliasName,
    depth: usize,
}

struct RequesterResolution {
    traversal: SourceClosureTraversal,
    requester: PackageSourceCustody,
    depth: usize,
    selected: Vec<SelectedDependency>,
}

/// The exact outstanding occurrence and its continuation. Consuming this value
/// supplies one result or fails the traversal; observing a request cannot skip it.
pub(crate) struct PendingSourceResolution {
    owner: RequesterResolution,
    purpose: DependencyPurpose,
    dependency_index: usize,
    dependency_depth: usize,
}

pub(crate) fn begin_source_closure<E>(
    root_request: PackageRootSourceRequest,
    root: PackageSourceCustody,
    limits: PackageSourceClosureLimits,
) -> Result<SourceClosureStep, PackageSourceClosureResolutionError<E>> {
    if limits.max_packages == 0 {
        return Err(PackageSourceClosureResolutionError::LimitExceeded {
            kind: PackageSourceClosureLimitKind::Packages,
            limit: limits.max_packages,
        });
    }
    let root_key = root.key().clone();
    let root_role = root.role();
    let accepted = BTreeMap::from([(root_key.clone(), root.clone())]);
    let acquired = BTreeMap::from([(root_key.source_lineage().clone(), root.clone())]);
    let observed = BTreeMap::from([(
        root_key.clone(),
        vec![ObservedCustody {
            custody: root,
            origins: vec![CustodyOrigin::Root],
        }],
    )]);
    SourceClosureTraversal {
        root_request,
        root_key: root_key.clone(),
        root_role,
        limits,
        accepted,
        acquired,
        observed,
        dependencies: BTreeMap::new(),
        depths: BTreeMap::from([(root_key.clone(), 0)]),
        dependency_request_count: 0,
        pending: VecDeque::from([root_key]),
    }
    .advance()
}

impl PendingSourceResolution {
    pub(crate) fn acquired_source(
        &self,
        lineage: &package_source::SourceLineage,
    ) -> Option<&PackageSourceCustody> {
        self.owner.traversal.acquired.get(lineage)
    }

    pub(crate) const fn purpose(&self) -> DependencyPurpose {
        self.purpose
    }

    pub(crate) const fn dependency_index(&self) -> usize {
        self.dependency_index
    }

    pub(crate) fn requester(&self) -> &PackageSourceCustody {
        &self.owner.requester
    }

    pub(crate) fn source(&self) -> &DependencySourceRequest {
        &self.owner.requester.dependency_requests(self.purpose)[self.dependency_index]
    }

    pub(crate) fn fail<E>(self, error: E) -> PackageSourceClosureResolutionError<E> {
        PackageSourceClosureResolutionError::Adapter {
            requester: self.owner.requester.key().clone(),
            purpose: self.purpose,
            dependency_index: self.dependency_index,
            request: self.source().clone(),
            error,
        }
    }

    pub(crate) fn complete<E>(
        mut self,
        dependency: PackageSourceCustody,
    ) -> Result<SourceClosureStep, PackageSourceClosureResolutionError<E>> {
        if dependency.role() != BuildDeclarationKind::Package {
            return Err(PackageSourceClosureResolutionError::InvalidDependencyRole {
                requester: self.owner.requester.key().clone(),
                purpose: self.purpose,
                dependency_index: self.dependency_index,
                selected: dependency.key().clone(),
                role: dependency.role(),
            });
        }
        let alias = self.source().resolved_alias(dependency.key().name());
        self.owner
            .traversal
            .acquired
            .entry(dependency.key().source_lineage().clone())
            .or_insert_with(|| dependency.clone());
        self.owner.selected.push(SelectedDependency {
            purpose: self.purpose,
            dependency_index: self.dependency_index,
            custody: dependency,
            alias,
            depth: self.dependency_depth,
        });
        self.owner.advance()
    }
}

impl SourceClosureTraversal {
    fn advance<E>(mut self) -> Result<SourceClosureStep, PackageSourceClosureResolutionError<E>> {
        while let Some(requester_key) = self.pending.pop_front() {
            let requester = self.accepted[&requester_key].clone();
            let depth = self.depths[&requester_key];
            let count = requester.dependency_projections().authored_request_count();
            let requester = RequesterResolution {
                traversal: self,
                requester,
                depth,
                selected: Vec::with_capacity(count),
            };
            if count != 0 {
                return requester.advance();
            }
            // Empty rosters need no external work. Drain them iteratively;
            // a wide frontier must not become a chain of recursive resumes.
            self = requester.join()?;
        }
        self.finish().map(SourceClosureStep::Complete)
    }

    fn finish<E>(
        self,
    ) -> Result<ResolvedPackageSourceClosure, PackageSourceClosureResolutionError<E>> {
        let conflicts = collect_conflicts(
            &self.root_key,
            &self.observed,
            &self.dependencies,
            &self.accepted,
        );
        if !conflicts.is_empty() {
            return Err(PackageSourceClosureResolutionError::ConflictingCustody { conflicts });
        }
        let nodes = self
            .accepted
            .values()
            .map(|custody| {
                ResolvedPackageNode::new(
                    ResolvedSourceIdentity::from_validated_parts(
                        custody.key().clone(),
                        custody.resolution().clone(),
                    ),
                    self.dependencies
                        .get(custody.key())
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .collect();
        let graph = ResolvedPackageClosure::new(self.root_key, self.root_role, nodes)
            .map_err(|errors| PackageSourceClosureResolutionError::InvalidClosure { errors })?;
        let custodies: Vec<_> = self.accepted.into_values().collect();
        let custody_indices = custodies
            .iter()
            .enumerate()
            .map(|(position, custody)| (custody.key().clone(), position))
            .collect();
        Ok(ResolvedPackageSourceClosure {
            root_request: self.root_request,
            graph,
            custodies,
            custody_indices,
        })
    }
}

impl RequesterResolution {
    fn advance<E>(mut self) -> Result<SourceClosureStep, PackageSourceClosureResolutionError<E>> {
        let mut position = self.selected.len();
        for purpose in DependencyPurpose::ALL {
            let count = self.requester.dependency_requests(purpose).len();
            if position >= count {
                position -= count;
                continue;
            }
            self.traversal.dependency_request_count =
                self.traversal.dependency_request_count.saturating_add(1);
            let limits = self.traversal.limits;
            if self.traversal.dependency_request_count > limits.max_dependency_requests {
                return Err(PackageSourceClosureResolutionError::LimitExceeded {
                    kind: PackageSourceClosureLimitKind::DependencyRequests,
                    limit: limits.max_dependency_requests,
                });
            }
            let dependency_depth = self.depth.saturating_add(1);
            if dependency_depth > limits.max_depth {
                return Err(PackageSourceClosureResolutionError::LimitExceeded {
                    kind: PackageSourceClosureLimitKind::Depth,
                    limit: limits.max_depth,
                });
            }
            return Ok(SourceClosureStep::Request(PendingSourceResolution {
                owner: self,
                purpose,
                dependency_index: position,
                dependency_depth,
            }));
        }
        self.join()?.advance()
    }

    fn join<E>(mut self) -> Result<SourceClosureTraversal, PackageSourceClosureResolutionError<E>> {
        let requester_key = self.requester.key().clone();
        for purpose in DependencyPurpose::ALL {
            let selected_package_names = self
                .selected
                .iter()
                .filter(|dependency| dependency.purpose == purpose)
                .map(|dependency| dependency.custody.key().name().clone())
                .collect::<Vec<_>>();
            self.requester
                .dependency_projections()
                .validate_aliases(purpose, &selected_package_names)
                .map_err(
                    |error| PackageSourceClosureResolutionError::InvalidAliases {
                        requester: requester_key.clone(),
                        purpose,
                        error,
                    },
                )?;
        }
        let mut resolved = Vec::with_capacity(self.selected.len());
        for selected in self.selected {
            let target = selected.custody.key().clone();
            resolved.push(ResolvedDependency::new(
                selected.purpose,
                selected.dependency_index,
                selected.alias.clone(),
                target.clone(),
            ));
            let origin = CustodyOrigin::Dependency {
                requester: requester_key.clone(),
                purpose: selected.purpose,
                dependency_index: selected.dependency_index,
                alias: selected.alias,
            };
            let candidates = self.traversal.observed.entry(target.clone()).or_default();
            if let Some(candidate) = candidates
                .iter_mut()
                .find(|candidate| candidate.custody.semantically_equivalent(&selected.custody))
            {
                candidate.origins.push(origin);
            } else {
                candidates.push(ObservedCustody {
                    custody: selected.custody.clone(),
                    origins: vec![origin],
                });
            }
            if !self.traversal.accepted.contains_key(&target) {
                if self.traversal.accepted.len() >= self.traversal.limits.max_packages {
                    return Err(PackageSourceClosureResolutionError::LimitExceeded {
                        kind: PackageSourceClosureLimitKind::Packages,
                        limit: self.traversal.limits.max_packages,
                    });
                }
                self.traversal
                    .accepted
                    .insert(target.clone(), selected.custody);
                self.traversal.depths.insert(target.clone(), selected.depth);
                self.traversal.pending.push_back(target);
            }
        }
        self.traversal.dependencies.insert(requester_key, resolved);
        Ok(self.traversal)
    }
}

fn collect_conflicts(
    root: &PackageKey,
    observed: &BTreeMap<PackageKey, Vec<ObservedCustody>>,
    dependencies: &BTreeMap<PackageKey, Vec<ResolvedDependency>>,
    custodies: &BTreeMap<PackageKey, PackageSourceCustody>,
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
                            paths_for_origin(root, origin, key, dependencies, custodies)
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
) -> Vec<DependencyRequestPath> {
    match origin {
        CustodyOrigin::Root => vec![DependencyRequestPath {
            root: root.clone(),
            steps: Vec::new(),
        }],
        CustodyOrigin::Dependency {
            requester,
            purpose,
            dependency_index,
            alias,
        } => {
            let mut requester_paths = paths_to_package(root, requester, dependencies, custodies);
            for path in &mut requester_paths {
                path.steps.push(DependencyRequestPathStep {
                    requester: requester.clone(),
                    purpose: *purpose,
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
        debug_assert_eq!(
            custodies[current]
                .dependency_projections()
                .authored_request_count(),
            outgoing.len()
        );
        for dependency in outgoing {
            if active.contains(dependency.target()) {
                continue;
            }
            steps.push(DependencyRequestPathStep {
                requester: current.clone(),
                purpose: dependency.purpose(),
                dependency_index: dependency.dependency_index(),
                alias: dependency.alias().clone(),
                target: dependency.target().clone(),
            });
            collect_paths(
                root,
                dependency.target(),
                target,
                dependencies,
                custodies,
                active,
                steps,
                paths,
            );
            steps.pop();
        }
    }

    active.remove(current);
}
