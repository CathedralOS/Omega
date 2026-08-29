//! Bounded traversal and reconciliation of the complete package source closure.

use super::graph::{
    PackageClosureValidationError, ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode,
    ResolvedSourceIdentity,
};
use crate::declarations::dependency_projection::DependencySourceRequest;
use crate::resolution::identity::{
    AliasName, ExternalSourceContext, ImmutableSourceResolution, PackageKey, SourceLineage,
    WorkspaceMemberPath,
};
use crate::source::{GitSourceRequest, LocalSourceLimits};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

/// Transport-erased custody for one resolved immutable package source.
///
/// There is deliberately no public constructor. Source adapters derive this
/// value from `ResolvedPackageSource<S>` only after source custody, package
/// declaration extraction, and hermetic dependency projection have succeeded.
#[derive(Debug, Clone)]
pub struct PackageSourceCustody {
    key: PackageKey,
    resolution: ImmutableSourceResolution,
    snapshot_root: PathBuf,
    /// Resolver work ceiling retained for later custody revalidation. This is
    /// operational policy, not package/source identity.
    source_limits: LocalSourceLimits,
    dependency_requests: Vec<DependencySourceRequest>,
}

#[cfg(test)]
mod tests;

/// The exact request that selected the root of one resolved source closure.
///
/// Dependency requests are authored in a requester's `build.omg` and remain in
/// that requester's custody. The root has no requester, so its request must be
/// retained separately instead of being inferred from normalized lineage or
/// immutable resolution after traversal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageRootSourceRequest {
    Git(GitSourceRequest),
    WorkspaceMember {
        workspace_root_source: SourceLineage,
        member_path: WorkspaceMemberPath,
        requested_workspace_root: PathBuf,
    },
    ExternalLocal {
        requested_root: PathBuf,
        source_context: ExternalSourceContext,
    },
}

impl PartialEq for PackageSourceCustody {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.resolution == other.resolution
            && self.snapshot_root == other.snapshot_root
            && self.dependency_requests == other.dependency_requests
    }
}

impl Eq for PackageSourceCustody {}

impl PackageSourceCustody {
    pub(crate) fn from_resolved_parts(
        key: PackageKey,
        resolution: ImmutableSourceResolution,
        snapshot_root: PathBuf,
        source_limits: LocalSourceLimits,
        dependency_requests: Vec<DependencySourceRequest>,
    ) -> Self {
        debug_assert!(resolution.matches_lineage(key.source_lineage()));
        Self {
            key,
            resolution,
            snapshot_root,
            source_limits,
            dependency_requests,
        }
    }

    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    pub fn snapshot_root(&self) -> &Path {
        &self.snapshot_root
    }

    pub fn source_limits(&self) -> LocalSourceLimits {
        self.source_limits
    }

    pub fn dependency_requests(&self) -> &[DependencySourceRequest] {
        &self.dependency_requests
    }

    fn source_identity(&self) -> ResolvedSourceIdentity {
        ResolvedSourceIdentity::from_validated_parts(self.key.clone(), self.resolution.clone())
    }
}

/// One exact requester-local edge in a root-to-dependency path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRequestPathStep {
    requester: PackageKey,
    dependency_index: usize,
    alias: AliasName,
    target: PackageKey,
}

impl DependencyRequestPathStep {
    pub fn requester(&self) -> &PackageKey {
        &self.requester
    }

    /// Zero-based position in the requester's projected dependency rows.
    pub fn dependency_index(&self) -> usize {
        self.dependency_index
    }

    pub fn alias(&self) -> &AliasName {
        &self.alias
    }

    pub fn target(&self) -> &PackageKey {
        &self.target
    }
}

/// One exact path by which source resolution discovered a package custody.
///
/// The root custody has an empty `steps` sequence. Dependency-row ordinals
/// keep repeated otherwise-identical authored requests distinguishable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRequestPath {
    root: PackageKey,
    steps: Vec<DependencyRequestPathStep>,
}

impl DependencyRequestPath {
    pub fn root(&self) -> &PackageKey {
        &self.root
    }

    pub fn steps(&self) -> &[DependencyRequestPathStep] {
        &self.steps
    }
}

/// One distinct custody observed for a conflicted `PackageKey`, together with
/// every dependency path that produced that exact custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceClosureConflictCandidate {
    custody: PackageSourceCustody,
    requesting_paths: Vec<DependencyRequestPath>,
}

impl PackageSourceClosureConflictCandidate {
    pub fn custody(&self) -> &PackageSourceCustody {
        &self.custody
    }

    pub fn requesting_paths(&self) -> &[DependencyRequestPath] {
        &self.requesting_paths
    }
}

/// All distinct source custodies observed for one conflicting package key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceClosureConflict {
    key: PackageKey,
    candidates: Vec<PackageSourceClosureConflictCandidate>,
}

/// Resolver-work ceilings applied across one complete source closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageSourceClosureLimits {
    pub max_packages: usize,
    pub max_dependency_requests: usize,
    pub max_depth: usize,
}

impl Default for PackageSourceClosureLimits {
    fn default() -> Self {
        Self {
            max_packages: 1024,
            max_dependency_requests: 16 * 1024,
            max_depth: 128,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSourceClosureLimitKind {
    Packages,
    DependencyRequests,
    Depth,
}

impl PackageSourceClosureConflict {
    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn candidates(&self) -> &[PackageSourceClosureConflictCandidate] {
        &self.candidates
    }
}

#[derive(Debug)]
pub enum PackageSourceClosureResolutionError<E> {
    /// The adapter could not resolve one projected dependency request.
    Adapter {
        requester: PackageKey,
        dependency_index: usize,
        request: DependencySourceRequest,
        error: E,
    },
    LimitExceeded {
        kind: PackageSourceClosureLimitKind,
        limit: usize,
    },
    /// One or more package keys produced non-identical immutable custody.
    ConflictingCustody {
        conflicts: Vec<PackageSourceClosureConflict>,
    },
    /// Final exact graph validation rejected the fully traversed closure.
    InvalidClosure {
        errors: Vec<PackageClosureValidationError>,
    },
}

impl<E> PackageSourceClosureResolutionError<E> {
    pub fn conflicts(&self) -> Option<&[PackageSourceClosureConflict]> {
        match self {
            Self::ConflictingCustody { conflicts } => Some(conflicts),
            Self::Adapter { .. } | Self::LimitExceeded { .. } | Self::InvalidClosure { .. } => None,
        }
    }
}

impl<E: fmt::Display> fmt::Display for PackageSourceClosureResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Adapter {
                requester,
                dependency_index,
                error,
                ..
            } => write!(
                formatter,
                "source adapter failed for dependency row {dependency_index} of package `{}`: {error}",
                requester.name().as_str()
            ),
            Self::LimitExceeded { kind, limit } => write!(
                formatter,
                "package source closure exceeded its {kind:?} limit of {limit}"
            ),
            Self::ConflictingCustody { conflicts } => write!(
                formatter,
                "source closure contains conflicting custody for {} package key(s)",
                conflicts.len()
            ),
            Self::InvalidClosure { errors } => write!(
                formatter,
                "resolved package source closure failed {} graph validation check(s)",
                errors.len()
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for PackageSourceClosureResolutionError<E> {}

/// A fully traversed and graph-validated source closure plus exact custody for
/// every package source root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPackageSourceClosure {
    root_request: PackageRootSourceRequest,
    graph: ResolvedPackageClosure,
    custodies: Vec<PackageSourceCustody>,
    custody_indices: BTreeMap<PackageKey, usize>,
}

/// One exact root request joined to the source identity it selected.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedRootPackageSourceRequest<'a> {
    request: &'a PackageRootSourceRequest,
    selected: &'a ResolvedSourceIdentity,
}

impl<'a> ResolvedRootPackageSourceRequest<'a> {
    pub fn request(&self) -> &'a PackageRootSourceRequest {
        self.request
    }

    pub fn selected(&self) -> &'a ResolvedSourceIdentity {
        self.selected
    }
}

/// One exact authored dependency request joined to the source it selected.
///
/// The request remains owned once by the requester's source custody. This view
/// binds it to the graph edge and target resolution without copying hostile
/// locator strings or choosing one primary request in a diamond graph.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedDependencySourceRequest<'a> {
    requester: &'a PackageKey,
    dependency_index: usize,
    request: &'a DependencySourceRequest,
    alias: &'a AliasName,
    selected: &'a ResolvedSourceIdentity,
}

impl<'a> ResolvedDependencySourceRequest<'a> {
    pub fn requester(&self) -> &'a PackageKey {
        self.requester
    }

    pub fn dependency_index(&self) -> usize {
        self.dependency_index
    }

    pub fn request(&self) -> &'a DependencySourceRequest {
        self.request
    }

    pub fn alias(&self) -> &'a AliasName {
        self.alias
    }

    pub fn selected(&self) -> &'a ResolvedSourceIdentity {
        self.selected
    }
}

/// A zero-copy, resolver-validated view of every source-selection occurrence.
///
/// This is source custody only. It is not compiler evidence, package admission,
/// a lock record, or a package instance.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedPackageSourceRequestSet<'a> {
    closure: &'a ResolvedPackageSourceClosure,
}

impl<'a> ResolvedPackageSourceRequestSet<'a> {
    pub fn root(&self) -> ResolvedRootPackageSourceRequest<'a> {
        let selected = self
            .closure
            .graph
            .package(self.closure.graph.root())
            .expect("validated closure contains its root package")
            .source();
        ResolvedRootPackageSourceRequest {
            request: &self.closure.root_request,
            selected,
        }
    }

    pub fn dependencies(&self) -> impl Iterator<Item = ResolvedDependencySourceRequest<'a>> + 'a {
        let closure = self.closure;
        closure.graph.packages().iter().flat_map(move |requester| {
            let requester_key = requester.source().key();
            let custody = closure
                .custody(requester_key)
                .expect("every validated graph package has source custody");
            debug_assert_eq!(
                requester.dependencies().len(),
                custody.dependency_requests().len()
            );
            requester.dependencies().iter().enumerate().map(
                move |(dependency_index, dependency)| {
                    let request = &custody.dependency_requests()[dependency_index];
                    let selected = closure
                        .graph
                        .package(dependency.target())
                        .expect("validated dependency edge has a target package")
                        .source();
                    ResolvedDependencySourceRequest {
                        requester: requester_key,
                        dependency_index,
                        request,
                        alias: dependency.alias(),
                        selected,
                    }
                },
            )
        })
    }
}

impl ResolvedPackageSourceClosure {
    pub fn source_requests(&self) -> ResolvedPackageSourceRequestSet<'_> {
        ResolvedPackageSourceRequestSet { closure: self }
    }

    pub fn graph(&self) -> &ResolvedPackageClosure {
        &self.graph
    }

    pub fn custodies(&self) -> &[PackageSourceCustody] {
        &self.custodies
    }

    pub fn custody(&self, key: &PackageKey) -> Option<&PackageSourceCustody> {
        self.custody_indices
            .get(key)
            .map(|index| &self.custodies[*index])
    }

    pub fn source_root(&self, key: &PackageKey) -> Option<&Path> {
        self.custody(key).map(PackageSourceCustody::snapshot_root)
    }

    /// One deterministic shortest root-to-package request path.
    ///
    /// Review evidence needs a useful explanation path, not the potentially
    /// exponential set of every path through a diamond-shaped DAG. Breadth-
    /// first traversal follows each requester's authored dependency order and
    /// visits every package at most once.
    pub fn dependency_path(&self, target: &PackageKey) -> Option<DependencyRequestPath> {
        self.custody(target)?;
        let root = self.graph.root();
        if root == target {
            return Some(DependencyRequestPath {
                root: root.clone(),
                steps: Vec::new(),
            });
        }

        let mut pending = VecDeque::from([root.clone()]);
        let mut visited = BTreeSet::from([root.clone()]);
        let mut predecessors = BTreeMap::<PackageKey, DependencyRequestPathStep>::new();
        while let Some(requester) = pending.pop_front() {
            let node = self
                .graph
                .package(&requester)
                .expect("validated closure traversal contains only package nodes");
            for (dependency_index, dependency) in node.dependencies().iter().enumerate() {
                if !visited.insert(dependency.target().clone()) {
                    continue;
                }
                predecessors.insert(
                    dependency.target().clone(),
                    DependencyRequestPathStep {
                        requester: requester.clone(),
                        dependency_index,
                        alias: dependency.alias().clone(),
                        target: dependency.target().clone(),
                    },
                );
                if dependency.target() == target {
                    let mut steps = Vec::new();
                    let mut current = target.clone();
                    while &current != root {
                        let step = predecessors
                            .get(&current)
                            .expect("discovered package has a predecessor")
                            .clone();
                        current = step.requester.clone();
                        steps.push(step);
                    }
                    steps.reverse();
                    return Some(DependencyRequestPath {
                        root: root.clone(),
                        steps,
                    });
                }
                pending.push_back(dependency.target().clone());
            }
        }

        None
    }
}

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
    let root_key = root.key.clone();
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
        let mut resolved_dependencies = Vec::with_capacity(requester.dependency_requests.len());

        for (dependency_index, request) in requester.dependency_requests.iter().enumerate() {
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
            let alias = request.resolved_alias(dependency.key.name());
            let target = dependency.key.clone();

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
        .map(|(index, custody)| (custody.key.clone(), index))
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
