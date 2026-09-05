//! Projection, recovery, validation, and canonical finalization.

use super::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectLimits,
    SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION, SOURCE_CLOSURE_SUBJECT_MAGIC,
};
use crate::declarations::dependencies::read::ProjectedDependencies;
use crate::resolution::graph::{ExactTargetPackageSourceClosure, ResolvedSourceIdentity};
use crate::resolution::source::PackageSourceNavigation;
use omega_target::TargetProfile;

use super::super::encoding::{
    Decoder, decode_dependency_projection, decode_dependency_selection, decode_package_navigation,
    decode_root_selection, decode_source_identity, decode_target_profile,
    encode_subject_with_budget, fingerprint,
};
use super::super::usage::Budget;
use super::super::validation::{canonical_root_request, validate_subject_with_budget};

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
        validate_subject_with_budget(
            &self.root,
            &self.packages,
            &self.package_navigations,
            &self.package_dependency_projections,
            &self.dependency_requests,
            limits,
            &mut Budget::new(usize::MAX),
        )
    }

    pub fn from_resolved(
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
                    .projected_dependencies()
                    .clone()
            })
            .collect::<Vec<_>>();
        let mut dependency_requests = closure
            .source_requests()
            .dependencies()
            .map(|selection| CanonicalDependencySourceSelection {
                requester: selection.requester().clone(),
                dependency_index: selection.dependency_index(),
                request: CanonicalDependencySourceRequest::from(selection.request()),
                alias: selection.alias().clone(),
                selected: selection.selected().clone(),
            })
            .collect::<Vec<_>>();
        dependency_requests.sort_by(|left, right| {
            left.requester
                .cmp(&right.requester)
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

    pub fn recover(
        bytes: &[u8],
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        let limits = limits.compiler_bounded();
        if bytes.len() > limits.maximum_record_bytes {
            return Err(CanonicalSourceClosureSubjectError::new(
                "source-closure subject exceeds its record-byte limit",
            ));
        }
        let mut decoder = Decoder::new(bytes);
        decoder.expect_fixed(SOURCE_CLOSURE_SUBJECT_MAGIC)?;
        if decoder.u16()? != SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION {
            return Err(CanonicalSourceClosureSubjectError::new(
                "unsupported source-closure subject version",
            ));
        }
        let target_profile = decode_target_profile(&mut decoder, limits)?;
        let root = decode_root_selection(&mut decoder, limits)?;
        let package_count = decoder.count(limits.maximum_packages)?;
        let mut packages = Vec::with_capacity(package_count);
        let mut package_navigations = Vec::with_capacity(package_count);
        let mut package_dependency_projections = Vec::with_capacity(package_count);
        for _ in 0..package_count {
            packages.push(decode_source_identity(
                &mut decoder,
                limits.maximum_identity_bytes,
            )?);
            package_navigations.push(decode_package_navigation(
                &mut decoder,
                limits.maximum_request_bytes,
            )?);
            package_dependency_projections
                .push(decode_dependency_projection(&mut decoder, limits)?);
        }
        let request_count = decoder.count(limits.maximum_dependency_requests)?;
        let mut dependency_requests = Vec::with_capacity(request_count);
        for _ in 0..request_count {
            dependency_requests.push(decode_dependency_selection(&mut decoder, limits)?);
        }
        decoder.finish()?;
        let recovered = Self::finish_with_projections(
            target_profile,
            root,
            packages,
            package_navigations,
            package_dependency_projections,
            dependency_requests,
            limits,
        )?;
        if recovered.canonical_bytes != bytes {
            return Err(CanonicalSourceClosureSubjectError::new(
                "source-closure subject is not canonically encoded",
            ));
        }
        Ok(recovered)
    }

    /// Require a recovered subject to equal a fresh projection from the exact
    /// current resolver custody. Decode or fingerprint equality alone is never
    /// enough for this comparison.
    pub fn matches_resolved(
        &self,
        target_closure: &ExactTargetPackageSourceClosure<'_>,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<bool, CanonicalSourceClosureSubjectError> {
        Ok(self == &Self::from_resolved(target_closure, limits)?)
    }

    pub(in super::super) fn finish_with_projections(
        target_profile: TargetProfile,
        root: CanonicalRootSourceSelection,
        packages: Vec<ResolvedSourceIdentity>,
        package_navigations: Vec<PackageSourceNavigation>,
        package_dependency_projections: Vec<ProjectedDependencies>,
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

    pub(in super::super) fn finish_with_budget(
        target_profile: TargetProfile,
        root: CanonicalRootSourceSelection,
        packages: Vec<ResolvedSourceIdentity>,
        package_navigations: Vec<PackageSourceNavigation>,
        package_dependency_projections: Vec<ProjectedDependencies>,
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
            canonical_bytes,
            fingerprint,
        })
    }
}
