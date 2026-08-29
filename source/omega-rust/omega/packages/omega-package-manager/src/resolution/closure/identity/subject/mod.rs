//! Canonical source-closure subject model and construction.

mod error;
mod fingerprint;
mod limits;
mod request;

pub use error::CanonicalSourceClosureSubjectError;
pub use fingerprint::CanonicalSourceClosureSubjectFingerprint;
pub use limits::CanonicalSourceClosureSubjectLimits;
pub use request::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection,
};

use super::super::{ResolvedPackageSourceClosure, ResolvedSourceIdentity};
use super::codec::{
    Decoder, decode_dependency_projection, decode_dependency_selection, decode_root_selection,
    decode_source_identity, decode_target_profile, encode_subject, fingerprint,
};
use super::validation::{canonical_root_request, validate_subject};
use crate::identity::PackageKey;
use crate::manifest::BuildDeclarationKind;
#[cfg(test)]
use crate::manifest::dependencies::read::DependencySourceRequest;
use crate::manifest::dependencies::read::ProjectedDependencies;
use crate::resolution::source::PackageSourceNavigation;
use omega_target::TargetProfile;

#[cfg(test)]
mod tests;

pub(super) const SOURCE_CLOSURE_SUBJECT_MAGIC: &[u8] = b"OMEGA-SOURCE-CLOSURE-SUBJECT\0";
pub const SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION: u16 = 5;
pub(super) const SOURCE_CLOSURE_SUBJECT_FINGERPRINT_DOMAIN: &[u8] =
    b"OMEGA-SOURCE-CLOSURE-SUBJECT-FINGERPRINT\0";

/// Canonical, non-admitting subject for one exact resolved source closure.
///
/// The subject binds every package key and immutable resolution, the exact root
/// request, and every requester-local dependency request occurrence. Snapshot
/// roots, cache paths, transport execution observations, compiler evidence,
/// certificates, decisions, and artifacts are intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSourceClosureSubject {
    pub(super) target_profile: TargetProfile,
    pub(super) root: CanonicalRootSourceSelection,
    pub(super) packages: Vec<ResolvedSourceIdentity>,
    pub(super) package_navigations: Vec<PackageSourceNavigation>,
    pub(super) package_dependency_projections: Vec<ProjectedDependencies>,
    pub(super) dependency_requests: Vec<CanonicalDependencySourceSelection>,
    pub(super) canonical_bytes: Vec<u8>,
    pub(super) fingerprint: CanonicalSourceClosureSubjectFingerprint,
}

impl CanonicalSourceClosureSubject {
    pub fn from_resolved(
        closure: &ResolvedPackageSourceClosure,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
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
            closure.target_profile(),
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
            package_navigations.push(super::codec::decode_package_navigation(
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

    pub const fn root(&self) -> &CanonicalRootSourceSelection {
        &self.root
    }

    pub const fn target_profile(&self) -> TargetProfile {
        self.target_profile
    }

    pub const fn root_role(&self) -> BuildDeclarationKind {
        self.root.role()
    }

    pub fn packages(&self) -> &[ResolvedSourceIdentity] {
        &self.packages
    }

    pub fn package_navigation(&self, package: &PackageKey) -> Option<&PackageSourceNavigation> {
        self.packages
            .binary_search_by(|source| source.key().cmp(package))
            .ok()
            .map(|index| &self.package_navigations[index])
    }

    pub fn package_dependency_projection(
        &self,
        package: &PackageKey,
    ) -> Option<&ProjectedDependencies> {
        self.packages
            .binary_search_by(|source| source.key().cmp(package))
            .ok()
            .map(|index| &self.package_dependency_projections[index])
    }

    pub fn dependency_requests(&self) -> &[CanonicalDependencySourceSelection] {
        &self.dependency_requests
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn fingerprint(&self) -> &CanonicalSourceClosureSubjectFingerprint {
        &self.fingerprint
    }

    /// Require a recovered subject to equal a fresh projection from the exact
    /// current resolver custody. Decode or fingerprint equality alone is never
    /// enough for this comparison.
    pub fn matches_resolved(
        &self,
        closure: &ResolvedPackageSourceClosure,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<bool, CanonicalSourceClosureSubjectError> {
        Ok(self == &Self::from_resolved(closure, limits)?)
    }

    #[cfg(test)]
    fn finish(
        root: CanonicalRootSourceSelection,
        packages: Vec<ResolvedSourceIdentity>,
        package_navigations: Vec<PackageSourceNavigation>,
        dependency_requests: Vec<CanonicalDependencySourceSelection>,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        let package_dependency_projections =
            unconditional_projections(&packages, &dependency_requests)?;
        Self::finish_with_projections(
            TargetProfile::CrossPlatformCli,
            root,
            packages,
            package_navigations,
            package_dependency_projections,
            dependency_requests,
            limits,
        )
    }

    fn finish_with_projections(
        target_profile: TargetProfile,
        root: CanonicalRootSourceSelection,
        packages: Vec<ResolvedSourceIdentity>,
        package_navigations: Vec<PackageSourceNavigation>,
        package_dependency_projections: Vec<ProjectedDependencies>,
        dependency_requests: Vec<CanonicalDependencySourceSelection>,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        let limits = limits.compiler_bounded();
        validate_subject(
            target_profile,
            &root,
            &packages,
            &package_navigations,
            &package_dependency_projections,
            &dependency_requests,
            limits,
        )?;
        let canonical_bytes = encode_subject(
            target_profile,
            &root,
            &packages,
            &package_navigations,
            &package_dependency_projections,
            &dependency_requests,
            limits,
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

#[cfg(test)]
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

#[cfg(test)]
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
