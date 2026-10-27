//! Projection, recovery, validation, and canonical finalization.

use super::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectLimits,
};
use crate::declarations::dependencies::read::DependencyProjections;
use crate::resolution::graph::{ExactTargetPackageSourceClosure, ResolvedSourceIdentity};
use crate::resolution::source::PackageSourceNavigation;
use target::TargetProfile;

use super::encoding::{encode_subject_with_budget, fingerprint};
use super::usage::Budget;
use super::validation::{canonical_root_request, validate_subject_with_budget};

impl CanonicalSourceClosureSubject {
    /// Recheck caller limits over retained typed fields without recovering or
    /// cloning a second subject. This is a size/shape check, not acquisition.
    pub(crate) fn validate_recovery_limits(
        &self,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<(), CanonicalSourceClosureSubjectError> {
        let limits = limits.compiler_bounded();
        if self.canonical_bytes.len() > limits.maximum_record_bytes {
            return Err(CanonicalSourceClosureSubjectError::new(
                "source-closure subject exceeds its record-byte limit",
            ));
        }
        let mut budget = Budget::new(usize::MAX);
        validate_subject_with_budget(
            &self.root,
            &self.packages,
            &self.package_navigations,
            &self.package_dependency_projections,
            &self.dependency_requests,
            limits,
            &mut budget,
        )?;
        super::catalog::validate(self, &self.catalog_dependencies, limits, &mut budget)
    }

    pub(crate) fn from_resolved(
        target_closure: &ExactTargetPackageSourceClosure<'_>,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        let limits = limits.compiler_bounded();
        let subject = Self::from_resolved_without_catalog(target_closure, limits)?;
        let dependencies = super::catalog::project(target_closure, limits)?;
        subject.with_catalog_dependencies(dependencies, limits, &mut Budget::new(usize::MAX))
    }

    pub(super) fn from_resolved_without_catalog(
        target_closure: &ExactTargetPackageSourceClosure<'_>,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        let closure = target_closure.source_closure();
        let root_view = closure.source_requests().root();
        let root = CanonicalRootSourceSelection {
            request: canonical_root_request(root_view.request()),
            role: closure.root_role(),
            selected: root_view.selected().clone(),
        };
        let mut packages = closure
            .graph()
            .packages()
            .iter()
            .map(|package| package.source().clone())
            .collect::<Vec<_>>();
        packages.sort_by(|left, right| left.key().cmp(right.key()));
        let package_navigations = packages
            .iter()
            .map(|package| {
                closure
                    .custody(package.key())
                    .expect("validated closure retains every package custody")
                    .navigation()
                    .clone()
            })
            .collect::<Vec<_>>();
        let package_dependency_projections = packages
            .iter()
            .map(|package| {
                closure
                    .custody(package.key())
                    .expect("validated closure retains every package custody")
                    .dependency_projections()
                    .clone()
            })
            .collect::<Vec<_>>();
        let mut dependency_requests = closure
            .source_requests()
            .dependencies()
            .map(|selection| CanonicalDependencySourceSelection {
                requester: selection.requester().clone(),
                purpose: selection.purpose(),
                dependency_index: selection.dependency_index(),
                request: CanonicalDependencySourceRequest::from(selection.request()),
                alias: selection.alias().clone(),
                selected: selection.selected().clone(),
            })
            .collect::<Vec<_>>();
        dependency_requests.sort_by(|left, right| {
            left.requester
                .cmp(&right.requester)
                .then(left.purpose.cmp(&right.purpose))
                .then(left.dependency_index.cmp(&right.dependency_index))
        });
        Self::finish_with_projections(
            target_closure.target_profile(),
            root,
            packages,
            package_navigations,
            package_dependency_projections,
            dependency_requests,
            limits,
        )
    }

    /// Require a recovered subject to equal a fresh projection from the exact
    /// current resolver custody. Decode or fingerprint equality alone is never
    /// enough for this comparison.
    pub(crate) fn matches_resolved(
        &self,
        target_closure: &ExactTargetPackageSourceClosure<'_>,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<bool, CanonicalSourceClosureSubjectError> {
        Ok(self == &Self::from_resolved(target_closure, limits)?)
    }

    pub(super) fn finish_with_projections(
        target_profile: TargetProfile,
        root: CanonicalRootSourceSelection,
        packages: Vec<ResolvedSourceIdentity>,
        package_navigations: Vec<PackageSourceNavigation>,
        package_dependency_projections: Vec<DependencyProjections>,
        dependency_requests: Vec<CanonicalDependencySourceSelection>,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        Self::finish_with_budget(
            target_profile,
            root,
            packages,
            package_navigations,
            package_dependency_projections,
            dependency_requests,
            limits,
            &mut Budget::new(usize::MAX),
        )
    }

    pub(super) fn finish_with_budget(
        target_profile: TargetProfile,
        root: CanonicalRootSourceSelection,
        packages: Vec<ResolvedSourceIdentity>,
        package_navigations: Vec<PackageSourceNavigation>,
        package_dependency_projections: Vec<DependencyProjections>,
        dependency_requests: Vec<CanonicalDependencySourceSelection>,
        limits: CanonicalSourceClosureSubjectLimits,
        budget: &mut Budget,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        let limits = limits.compiler_bounded();
        validate_subject_with_budget(
            &root,
            &packages,
            &package_navigations,
            &package_dependency_projections,
            &dependency_requests,
            limits,
            budget,
        )?;
        let canonical_bytes = encode_subject_with_budget(
            target_profile,
            &root,
            &packages,
            &package_navigations,
            &package_dependency_projections,
            &dependency_requests,
            &[],
            limits,
            budget,
        )?;
        if canonical_bytes.len() > limits.maximum_record_bytes {
            return Err(CanonicalSourceClosureSubjectError::new(
                "source-closure subject exceeds its record-byte limit",
            ));
        }
        let fingerprint = fingerprint(&canonical_bytes);
        Ok(Self {
            target_profile,
            root,
            packages,
            package_navigations,
            package_dependency_projections,
            dependency_requests,
            catalog_dependencies: Vec::new(),
            canonical_bytes,
            fingerprint,
        })
    }

    pub(super) fn with_catalog_dependencies(
        mut self,
        dependencies: Vec<super::CanonicalCatalogDependency>,
        limits: CanonicalSourceClosureSubjectLimits,
        budget: &mut Budget,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        super::catalog::validate(&self, &dependencies, limits, budget)?;
        if dependencies.is_empty() {
            return Ok(self);
        }
        let bytes = encode_subject_with_budget(
            self.target_profile,
            &self.root,
            &self.packages,
            &self.package_navigations,
            &self.package_dependency_projections,
            &self.dependency_requests,
            &dependencies,
            limits,
            budget,
        )?;
        if bytes.len() > limits.maximum_record_bytes {
            return Err(CanonicalSourceClosureSubjectError::new(
                "source-closure subject exceeds its record-byte limit",
            ));
        }
        self.fingerprint = fingerprint(&bytes);
        self.canonical_bytes = bytes;
        self.catalog_dependencies = dependencies;
        Ok(self)
    }
}
