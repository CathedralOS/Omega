//! Canonical commitments for exact review conflicts and candidate closures.

use super::format::{
    change_tag, row_kind_tag, row_risk_tag, source_location_role_tag, synthetic_source_kind_tag,
};
use super::model::*;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, DependencyRequestPath,
    ResolvedPackageClosure, ResolvedPackageSourceClosure,
};
use crate::manifest::PackageKey;
use crate::manifest::BuildDeclarationKind;
use crate::review::candidate::PackageReviewEvidence;
use omega_package_evidence::evidence::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
    PackageReviewSourceLocationOwner,
};
use omega_package_source::ImmutableSourceResolution;
use sha2::{Digest, Sha256};

const CONFLICT_FINGERPRINT_DOMAIN: &[u8] = b"OMEGA-PACKAGE-CAPABILITY-CONFLICT\0";
const CONFLICT_FINGERPRINT_VERSION: u16 = 17;
const CANDIDATE_CLOSURE_DOMAIN: &[u8] = b"OMEGA-PACKAGE-CANDIDATE-CLOSURE\0";
const CANDIDATE_CLOSURE_VERSION: u16 = 5;

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_conflict_fingerprint<B: PackageReviewEvidence, C: PackageReviewEvidence>(
    key: &PackageKey,
    baseline_review: &B,
    candidate_review: &C,
    dependency_path: &DependencyRequestPath,
    candidate_closure: ReviewOnlyCandidateClosureCommitment,
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    change: ReviewOnlyCapabilityConflictChange,
    row_key: &[u8],
    baseline_row: Option<&[u8]>,
    candidate_row: Option<&[u8]>,
    baseline_source: Option<&PackageReviewCanonicalRowSource>,
    candidate_source: Option<&PackageReviewCanonicalRowSource>,
) -> ReviewOnlyCapabilityConflictFingerprint {
    let mut digest = Sha256::new();
    hash_field(&mut digest, CONFLICT_FINGERPRINT_DOMAIN);
    digest.update(CONFLICT_FINGERPRINT_VERSION.to_le_bytes());
    hash_field(&mut digest, &key.identity().digest());
    hash_resolution(&mut digest, baseline_review.resolution());
    hash_resolution(&mut digest, candidate_review.resolution());
    hash_field(
        &mut digest,
        &PackageReviewEvidence::source_consumption_commitment(baseline_review).digest(),
    );
    hash_field(
        &mut digest,
        &PackageReviewEvidence::source_consumption_commitment(candidate_review).digest(),
    );
    hash_field(&mut digest, &baseline_review.whole_review_commitment());
    hash_field(&mut digest, &candidate_review.whole_review_commitment());
    hash_field(&mut digest, &candidate_closure.digest());
    hash_dependency_path(&mut digest, dependency_path);
    digest.update([row_kind_tag(kind), row_risk_tag(risk), change_tag(change)]);
    hash_field(&mut digest, row_key);
    hash_optional_field(&mut digest, baseline_row);
    hash_optional_field(&mut digest, candidate_row);
    hash_optional_row_source(&mut digest, baseline_source);
    hash_optional_row_source(&mut digest, candidate_source);
    ReviewOnlyCapabilityConflictFingerprint(digest.finalize().into())
}

pub(super) fn derive_candidate_closure_commitment<C: PackageReviewEvidence>(
    closure: &ResolvedPackageSourceClosure,
    candidate_reviews: &[&C],
) -> Result<ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflictError> {
    let source_closure = CanonicalSourceClosureSubject::from_resolved(
        closure,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .map_err(|_| ReviewOnlyCapabilityConflictError::InvalidCandidateSourceClosure)?;
    derive_candidate_graph_commitment_with_source(
        closure.graph(),
        Some(source_closure.canonical_bytes()),
        candidate_reviews,
    )
}

#[cfg(test)]
pub(super) fn derive_candidate_graph_commitment<C: PackageReviewEvidence>(
    closure: &ResolvedPackageClosure,
    candidate_reviews: &[&C],
) -> Result<ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflictError> {
    derive_candidate_graph_commitment_with_source(closure, None, candidate_reviews)
}

fn derive_candidate_graph_commitment_with_source<C: PackageReviewEvidence>(
    closure: &ResolvedPackageClosure,
    source_closure: Option<&[u8]>,
    candidate_reviews: &[&C],
) -> Result<ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflictError> {
    let mut digest = Sha256::new();
    hash_field(&mut digest, CANDIDATE_CLOSURE_DOMAIN);
    digest.update(CANDIDATE_CLOSURE_VERSION.to_le_bytes());
    match source_closure {
        Some(source_closure) => {
            digest.update([1]);
            hash_field(&mut digest, source_closure);
        }
        None => digest.update([0]),
    }
    hash_field(&mut digest, &closure.root().identity().digest());
    digest.update([root_role_tag(closure.root_role())]);
    let mut packages = Vec::new();
    packages
        .try_reserve(closure.packages().len())
        .map_err(|_| ReviewOnlyCapabilityConflictError::AllocationFailed)?;
    packages.extend(closure.packages());
    packages.sort_by(|left, right| left.source().key().cmp(right.source().key()));
    digest.update(
        u64::try_from(packages.len())
            .expect("bounded package count fits u64")
            .to_le_bytes(),
    );
    for package in packages {
        let review_index = candidate_reviews
            .binary_search_by(|review| review.key().cmp(package.source().key()))
            .expect("validated candidate closure has one review per source");
        let review = candidate_reviews[review_index];
        hash_field(&mut digest, &package.source().key().identity().digest());
        hash_resolution(&mut digest, package.source().resolution());
        hash_field(&mut digest, review.target_name().as_bytes());
        hash_field(
            &mut digest,
            &review.source_consumption_commitment().digest(),
        );
        match review.build_observation_commitment() {
            None => digest.update([0]),
            Some(commitment) => {
                digest.update([1]);
                hash_field(&mut digest, &commitment);
            }
        }
        hash_field(&mut digest, &review.whole_review_commitment());
        digest.update(
            u64::try_from(package.dependencies().len())
                .expect("bounded dependency count fits u64")
                .to_le_bytes(),
        );
        for (dependency_index, dependency) in package.dependencies().iter().enumerate() {
            digest.update(
                u64::try_from(dependency_index)
                    .expect("bounded dependency index fits u64")
                    .to_le_bytes(),
            );
            hash_field(&mut digest, dependency.alias().as_str().as_bytes());
            hash_field(&mut digest, &dependency.target().identity().digest());
        }
    }
    Ok(ReviewOnlyCandidateClosureCommitment(
        digest.finalize().into(),
    ))
}

fn root_role_tag(role: BuildDeclarationKind) -> u8 {
    match role {
        BuildDeclarationKind::Package => 0,
        BuildDeclarationKind::Application => 1,
        BuildDeclarationKind::Workspace => {
            unreachable!("workspace catalogs cannot enter a resolved package closure")
        }
    }
}

fn hash_resolution(digest: &mut Sha256, resolution: &ImmutableSourceResolution) {
    match resolution {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => {
            digest.update([0]);
            hash_field(digest, commit.to_hex().as_bytes());
            hash_field(digest, tree.to_hex().as_bytes());
            hash_field(digest, content.to_hex().as_bytes());
        }
        ImmutableSourceResolution::Workspace { content } => {
            digest.update([1]);
            hash_field(digest, content.to_hex().as_bytes());
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            digest.update([2]);
            hash_field(digest, content.to_hex().as_bytes());
        }
    }
}

fn hash_dependency_path(digest: &mut Sha256, path: &DependencyRequestPath) {
    hash_field(digest, &path.root().identity().digest());
    digest.update(
        u64::try_from(path.steps().len())
            .expect("bounded dependency path length fits u64")
            .to_le_bytes(),
    );
    for step in path.steps() {
        hash_field(digest, &step.requester().identity().digest());
        digest.update(
            u64::try_from(step.dependency_index())
                .expect("dependency index fits u64")
                .to_le_bytes(),
        );
        hash_field(digest, step.alias().as_str().as_bytes());
        hash_field(digest, &step.target().identity().digest());
    }
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("canonical conflict field length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

fn hash_optional_field(digest: &mut Sha256, bytes: Option<&[u8]>) {
    match bytes {
        Some(bytes) => {
            digest.update([1]);
            hash_field(digest, bytes);
        }
        None => digest.update([0]),
    }
}

fn hash_optional_row_source(digest: &mut Sha256, source: Option<&PackageReviewCanonicalRowSource>) {
    match source {
        None => digest.update([0]),
        Some(source) => {
            digest.update([1]);
            let locations = source.authored_locations().unwrap_or_default();
            digest.update(
                u64::try_from(locations.len())
                    .expect("bounded source-location count fits u64")
                    .to_le_bytes(),
            );
            for location in locations {
                match location.owner() {
                    PackageReviewSourceLocationOwner::Package(package) => {
                        digest.update([0]);
                        hash_field(digest, &package.digest());
                    }
                    PackageReviewSourceLocationOwner::Toolchain(source) => {
                        digest.update([1]);
                        hash_field(digest, &source.digest());
                    }
                }
                hash_field(digest, location.relative_path().as_bytes());
                digest.update(location.start_byte().to_le_bytes());
                digest.update(location.end_byte().to_le_bytes());
                digest.update([source_location_role_tag(location.role())]);
            }
            digest.update(
                u64::try_from(source.compiler_derivations().len())
                    .expect("bounded compiler-derivation count fits u64")
                    .to_le_bytes(),
            );
            for kind in source.compiler_derivations() {
                digest.update([synthetic_source_kind_tag(*kind)]);
            }
        }
    }
}
