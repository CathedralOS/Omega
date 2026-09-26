//! Test-only construction of source subjects without resolver custody.

use super::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectLimits,
};
use crate::declarations::dependencies::read::{
    DependencyProjections, DependencyPurpose, DependencySourceRequest, ProjectedDependencies,
};
use crate::resolution::graph::ResolvedSourceIdentity;
use crate::resolution::source::PackageSourceNavigation;
use target::TargetProfile;

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
) -> Result<Vec<DependencyProjections>, CanonicalSourceClosureSubjectError> {
    packages
        .iter()
        .map(|package| {
            let mut scopes = [
                Vec::<DependencySourceRequest>::new(),
                Vec::<DependencySourceRequest>::new(),
            ];
            for purpose in DependencyPurpose::ALL {
                let rows = dependency_requests
                    .iter()
                    .filter(|selection| {
                        &selection.requester == package.key() && selection.purpose == purpose
                    })
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
                scopes[usize::from(purpose as u8)] = rows
                    .into_iter()
                    .map(|selection| projected_request(&selection.request))
                    .collect();
            }
            let [product, build] = scopes;
            Ok(DependencyProjections::new(
                ProjectedDependencies::from(product),
                ProjectedDependencies::from(build),
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
