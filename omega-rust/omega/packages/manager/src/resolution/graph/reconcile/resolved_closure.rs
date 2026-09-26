//! Validated closure custody and exact source-selection views.

use super::super::{ResolvedPackageClosure, ResolvedSourceIdentity};
use super::DependencyRequestPath;
use super::DependencyRequestPaths;
use crate::declarations::BuildDeclarationKind;
use crate::declarations::dependencies::read::{DependencyPurpose, DependencySourceRequest};
use crate::declarations::{AliasName, PackageKey};
use crate::resolution::graph::PackageRootSourceRequest;
use crate::resolution::source::PackageSourceCustody;
use target::TargetProfile;

use std::collections::BTreeMap;
use std::path::Path;

/// A fully traversed and graph-validated source closure plus exact custody for
/// every package source root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPackageSourceClosure {
    pub(super) root_request: PackageRootSourceRequest,
    pub(super) graph: ResolvedPackageClosure,
    pub(super) custodies: Vec<PackageSourceCustody>,
    pub(super) custody_indices: BTreeMap<PackageKey, usize>,
}

/// One target-specific child view over an already resolved source closure.
///
/// Source acquisition and dependency reconciliation happen once in the parent.
/// Target-conditioned build activation, checking, review, and production must
/// consume an explicit child so source custody cannot be confused with target
/// identity.
#[derive(Debug, Clone, Copy)]
pub struct ExactTargetPackageSourceClosure<'a> {
    source_closure: &'a ResolvedPackageSourceClosure,
    target_profile: TargetProfile,
}

impl<'a> ExactTargetPackageSourceClosure<'a> {
    pub const fn source_closure(&self) -> &'a ResolvedPackageSourceClosure {
        self.source_closure
    }

    pub const fn target_profile(&self) -> TargetProfile {
        self.target_profile
    }
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
    purpose: DependencyPurpose,
    dependency_index: usize,
    request: &'a DependencySourceRequest,
    alias: &'a AliasName,
    selected: &'a ResolvedSourceIdentity,
}

impl<'a> ResolvedDependencySourceRequest<'a> {
    pub fn requester(&self) -> &'a PackageKey {
        self.requester
    }

    /// Which authorized context this request occurrence belongs to.
    pub const fn purpose(&self) -> DependencyPurpose {
        self.purpose
    }

    /// Zero-based position in the requester's authored rows for this edge's
    /// purpose scope.
    pub const fn dependency_index(&self) -> usize {
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
                custody.dependency_projections().authored_request_count()
            );
            requester.dependencies().iter().map(move |dependency| {
                let request = &custody.dependency_requests(dependency.purpose())
                    [dependency.dependency_index()];
                let selected = closure
                    .graph
                    .package(dependency.target())
                    .expect("validated dependency edge has a target package")
                    .source();
                ResolvedDependencySourceRequest {
                    requester: requester_key,
                    purpose: dependency.purpose(),
                    dependency_index: dependency.dependency_index(),
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

    pub const fn for_exact_target(
        &self,
        target_profile: TargetProfile,
    ) -> ExactTargetPackageSourceClosure<'_> {
        ExactTargetPackageSourceClosure {
            source_closure: self,
            target_profile,
        }
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
        if target == self.graph.root() {
            return Some(DependencyRequestPath {
                root: target.clone(),
                steps: Vec::new(),
            });
        }
        DependencyRequestPaths::new(self, Some(target))?.path(target)
    }

    /// Prepare once when explaining several packages in this closure.
    pub(crate) fn dependency_paths(&self) -> Option<DependencyRequestPaths<'_>> {
        DependencyRequestPaths::new(self, None)
    }
}
