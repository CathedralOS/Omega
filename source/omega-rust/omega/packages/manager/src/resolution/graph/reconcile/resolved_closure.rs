//! Validated closure custody and exact source-selection views.

use super::super::{ResolvedPackageClosure, ResolvedSourceIdentity};
use super::model::{DependencyRequestPath, DependencyRequestPathStep};
use crate::declarations::BuildDeclarationKind;
use crate::declarations::dependencies::read::DependencySourceRequest;
use crate::declarations::{AliasName, PackageKey};
use crate::resolution::graph::PackageRootSourceRequest;
use crate::resolution::source::PackageSourceCustody;
use omega_target::TargetProfile;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

/// A fully traversed and graph-validated source closure plus exact custody for
/// every package source root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPackageSourceClosure {
    pub(super) root_request: PackageRootSourceRequest,
    pub(super) target_profile: TargetProfile,
    pub(super) graph: ResolvedPackageClosure,
    pub(super) custodies: Vec<PackageSourceCustody>,
    pub(super) custody_indices: BTreeMap<PackageKey, usize>,
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
            let active_occurrences = custody
                .projected_dependencies()
                .occurrence_indices_for_profile(closure.target_profile)
                .collect::<Vec<_>>();
            debug_assert_eq!(requester.dependencies().len(), active_occurrences.len());
            requester
                .dependencies()
                .iter()
                .enumerate()
                .map(move |(active_index, dependency)| {
                    let dependency_index = active_occurrences[active_index];
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
                })
        })
    }
}

impl ResolvedPackageSourceClosure {
    pub fn source_requests(&self) -> ResolvedPackageSourceRequestSet<'_> {
        ResolvedPackageSourceRequestSet { closure: self }
    }

    pub const fn target_profile(&self) -> TargetProfile {
        self.target_profile
    }

    pub fn graph(&self) -> &ResolvedPackageClosure {
        &self.graph
    }

    /// Exact role authored by the selected closure root.
    pub const fn root_role(&self) -> BuildDeclarationKind {
        self.graph.root_role()
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
            let custody = self
                .custody(&requester)
                .expect("validated graph package retains source custody");
            let active_occurrences = custody
                .projected_dependencies()
                .occurrence_indices_for_profile(self.target_profile)
                .collect::<Vec<_>>();
            for (active_index, dependency) in node.dependencies().iter().enumerate() {
                let dependency_index = active_occurrences[active_index];
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
