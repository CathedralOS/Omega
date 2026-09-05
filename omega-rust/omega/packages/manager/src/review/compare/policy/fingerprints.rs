//! Comparison identities bind both inert policy and fresh issuer provenance.
use super::{
    PackagePolicyChangeFingerprint, PackagePolicyChangeKind, PackagePolicyDependencyPath,
    PackagePolicyPackageChange, PackagePolicyReplacementSite,
};
use crate::declarations::PackageKey;
use crate::lock::PackageLockTarget;
use crate::resolution::graph::CanonicalSourceClosureSubject;
use crate::review::{
    CompilerIssuedPackageReview,
    candidate::{build_observation_commitment, whole_review_commitment},
};
use package_evidence::record::{PACKAGE_POLICY_ROW_VERSION, PackagePolicyRow};
use sha2::{Digest, Sha256};

pub(super) fn context(
    accepted: Option<&PackageLockTarget>,
    source: &CanonicalSourceClosureSubject,
) -> Sha256 {
    let mut hash = Sha256::new();
    field(&mut hash, b"OMEGA-PACKAGE-POLICY-COMPARISON-CONTEXT\0");
    hash.update(2_u16.to_le_bytes());
    hash.update(PACKAGE_POLICY_ROW_VERSION.to_le_bytes());
    hash.update([u8::from(accepted.is_some())]);
    if let Some(accepted) = accepted {
        field(&mut hash, accepted.source().canonical_bytes());
    }
    field(&mut hash, source.canonical_bytes());
    hash
}

pub(super) fn package_context(
    hash: &mut Sha256,
    key: &PackageKey,
    baseline_present: bool,
    baseline: &[PackagePolicyRow],
    candidate: Option<&CompilerIssuedPackageReview>,
    rows: &[PackagePolicyRow],
) {
    field(hash, &key.identity().digest());
    hash.update([u8::from(baseline_present)]);
    row_set(hash, baseline);
    hash.update([u8::from(candidate.is_some())]);
    row_set(hash, rows);
    if let Some(candidate) = candidate {
        field(hash, &candidate.source_consumption_commitment().digest());
        field(
            hash,
            &whole_review_commitment(candidate.canonical_review_bytes()),
        );
        field(hash, candidate.selected_build_machine_identity().as_bytes());
        hash.update([u8::from(candidate.build_observation_summary().is_some())]);
        if let Some(summary) = candidate.build_observation_summary() {
            field(hash, &build_observation_commitment(summary));
        }
    }
}

pub(super) fn finish_package(
    context: PackagePolicyChangeFingerprint,
    package: &mut PackagePolicyPackageChange,
) {
    let mut hash = Sha256::new();
    field(&mut hash, b"OMEGA-PACKAGE-POLICY-PACKAGE-CHANGE\0");
    hash.update(1_u16.to_le_bytes());
    field(&mut hash, &context.digest());
    field(&mut hash, &package.key.identity().digest());
    // The context contains both exact immutable source resolutions, including
    // absence. Paths and flags are also bound explicitly for report consumers.
    path(&mut hash, package.baseline_path.as_ref());
    path(&mut hash, package.candidate_path.as_ref());
    hash.update([
        u8::from(package.source_changed),
        u8::from(package.source_association_changed),
        u8::from(package.audit_recommended),
    ]);
    package.fingerprint = PackagePolicyChangeFingerprint(hash.finalize().into());
    for delta in &mut package.rows {
        let mut hash = Sha256::new();
        field(&mut hash, b"OMEGA-PACKAGE-POLICY-ROW-CHANGE\0");
        hash.update(1_u16.to_le_bytes());
        field(&mut hash, &package.fingerprint.digest());
        hash.update([match delta.change {
            PackagePolicyChangeKind::Added => 1,
            PackagePolicyChangeKind::Removed => 2,
            PackagePolicyChangeKind::Changed => 3,
        }]);
        hash.update([
            u8::from(delta.requires_decision),
            u8::from(delta.audit_recommended),
        ]);
        for value in [delta.baseline.as_ref(), delta.candidate.as_ref()] {
            hash.update([u8::from(value.is_some())]);
            if let Some(value) = value {
                row(&mut hash, value);
            }
        }
        delta.fingerprint = PackagePolicyChangeFingerprint(hash.finalize().into());
    }
}

fn row_set(hash: &mut Sha256, rows: &[PackagePolicyRow]) {
    hash.update((rows.len() as u64).to_le_bytes());
    for value in rows {
        row(hash, value);
    }
}

pub(super) fn source_replacement(
    context: PackagePolicyChangeFingerprint,
    site: &PackagePolicyReplacementSite,
    baseline: &PackageKey,
    candidate: &PackageKey,
) -> PackagePolicyChangeFingerprint {
    let mut hash = Sha256::new();
    field(&mut hash, b"OMEGA-PACKAGE-POLICY-SOURCE-REPLACEMENT\0");
    hash.update(1_u16.to_le_bytes());
    field(&mut hash, &context.digest());
    match site {
        PackagePolicyReplacementSite::Root => hash.update([0]),
        PackagePolicyReplacementSite::Dependency { requester, alias } => {
            hash.update([1]);
            field(&mut hash, &requester.identity().digest());
            field(&mut hash, alias.as_str().as_bytes());
        }
    }
    field(&mut hash, &baseline.identity().digest());
    field(&mut hash, &candidate.identity().digest());
    PackagePolicyChangeFingerprint(hash.finalize().into())
}
fn row(hash: &mut Sha256, value: &PackagePolicyRow) {
    hash.update([value.kind().canonical_tag()]);
    field(hash, value.key_bytes());
    field(hash, value.canonical_bytes());
    field(hash, value.canonical_text().as_bytes());
    hash.update([
        u8::from(value.initial_requires_decision()),
        u8::from(value.update_requires_decision()),
        u8::from(value.audit_recommended_when_present()),
        u8::from(value.audit_recommended_on_change()),
    ]);
}
fn path(hash: &mut Sha256, value: Option<&PackagePolicyDependencyPath>) {
    hash.update([u8::from(value.is_some())]);
    if let Some(value) = value {
        field(hash, &value.root.digest());
        hash.update((value.steps.len() as u64).to_le_bytes());
        for step in &value.steps {
            field(hash, &step.requester.digest());
            hash.update((step.dependency_index as u64).to_le_bytes());
            field(hash, step.alias.as_bytes());
            field(hash, &step.target.digest());
        }
    }
}
fn field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
