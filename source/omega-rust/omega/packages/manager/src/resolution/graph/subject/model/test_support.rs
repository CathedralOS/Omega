//! Test-only construction of source subjects without resolver custody.

use super::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectLimits,
};
use crate::declarations::dependencies::read::{DependencySourceRequest, ProjectedDependencies};
use crate::resolution::graph::ResolvedSourceIdentity;
use crate::resolution::source::PackageSourceNavigation;
use omega_target::TargetProfile;

impl CanonicalSourceClosureSubject {
    pub(super) fn finish(
        root: CanonicalRootSourceSelection,
        packages: Vec<ResolvedSourceIdentity>,
        package_navigations: Vec<PackageSourceNavigation>,
        dependency_requests: Vec<CanonicalDependencySourceSelection>,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        Self::finish_for_target(
            TargetProfile::CrossPlatformCli,
            root,
            packages,
            package_navigations,
            dependency_requests,
            limits,
        )
    }

    pub(super) fn finish_for_target(
        target_profile: TargetProfile,
        root: CanonicalRootSourceSelection,
        packages: Vec<ResolvedSourceIdentity>,
        package_navigations: Vec<PackageSourceNavigation>,
        dependency_requests: Vec<CanonicalDependencySourceSelection>,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        let package_dependency_projections =
            unconditional_projections(&packages, &dependency_requests)?;
        Self::finish_with_projections(
            target_profile,
            root,
            packages,
            package_navigations,
            package_dependency_projections,
            dependency_requests,
            limits,
        )
    }
}

fn unconditional_projections(
    packages: &[ResolvedSourceIdentity],
    dependency_requests: &[CanonicalDependencySourceSelection],
) -> Result<Vec<ProjectedDependencies>, CanonicalSourceClosureSubjectError> {
    packages
        .iter()
        .map(|package| {
            let rows = dependency_requests
                .iter()
                .filter(|selection| &selection.requester == package.key())
                .collect::<Vec<_>>();
            for (expected, row) in rows.iter().enumerate() {
                if row.dependency_index != expected {
                    return Err(CanonicalSourceClosureSubjectError::new(if expected == 0 {
                        "dependency request ordinals do not begin at zero"
                    } else {
                        "dependency request ordinals are not contiguous"
                    }));
                }
            }
            Ok(ProjectedDependencies::from(
                rows.into_iter()
                    .map(|selection| projected_request(&selection.request))
                    .collect::<Vec<_>>(),
            ))
        })
        .collect()
}

fn projected_request(request: &CanonicalDependencySourceRequest) -> DependencySourceRequest {
    match request {
        CanonicalDependencySourceRequest::Path {
            explicit_alias,
            location,
        } => DependencySourceRequest::Path {
            explicit_alias: explicit_alias.clone(),
            location: location.clone(),
        },
        CanonicalDependencySourceRequest::Git {
            explicit_alias,
            repository,
            revision,
            selection,
        } => DependencySourceRequest::Git {
            explicit_alias: explicit_alias.clone(),
            repository: repository.clone(),
            revision: revision.clone(),
            selection: selection.clone(),
        },
    }
}
