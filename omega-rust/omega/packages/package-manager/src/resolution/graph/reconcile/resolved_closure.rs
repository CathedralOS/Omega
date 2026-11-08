// We expose validated closure custody and zero-copy source-selection views.
// Source acquisition and dependency reconciliation happen once in
// `ResolvedPackageSourceClosure`; target-conditioned build activation,
// checking, review, and production consume `ExactTargetPackageSourceClosure` so
// source custody cannot be confused with target identity. Dependency request
// views borrow the requester's authored request without copying locator strings
// or choosing a single primary request in a diamond graph, and `dependency_path`
// computes a deterministic shortest root-to-package explanation path via BFS.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedPackageSourceClosure {
    pub(super) root_request: PackageRootSourceRequest,
    pub(super) graph: ResolvedPackageClosure,
    pub(super) custodies: Vec<PackageSourceCustody>,
    pub(super) custody_indices: BTreeMap<PackageKey, usize>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ExactTargetPackageSourceClosure<'a> {
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedRootPackageSourceRequest<'a> {
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedDependencySourceRequest<'a> {
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

    pub const fn purpose(&self) -> DependencyPurpose {
        self.purpose
    }

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

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedPackageSourceRequestSet<'a> {
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
    pub(crate) fn source_requests(&self) -> ResolvedPackageSourceRequestSet<'_> {
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

    pub(crate) fn dependency_path(&self, target: &PackageKey) -> Option<DependencyRequestPath> {
        self.custody(target)?;
        if target == self.graph.root() {
            return Some(DependencyRequestPath {
                root: target.clone(),
                steps: Vec::new(),
            });
        }
        DependencyRequestPaths::new(self, Some(target))?.path(target)
    }
}
