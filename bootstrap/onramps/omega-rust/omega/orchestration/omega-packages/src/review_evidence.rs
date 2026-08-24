use crate::{CompilerIssuedPackageReview, ImmutableSourceResolution, PackageKey};
use omega_compiler::{
    BuildFilesystemGrantAccess, BuildFilesystemGrantRefusalReason, BuildFilesystemProvider,
    BuildObservationClass, BuildObservationSummary, CompilerExecutableCommitment,
    DecodedPackageReviewCanonicalRow, PackageReviewCanonicalRow, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
    PackageSourceConsumptionCommitment,
};
use sha2::{Digest, Sha256};

const WHOLE_REVIEW_COMMITMENT_DOMAIN: &[u8] = b"OMEGA-PACKAGE-REVIEW-COMPARISON\0";
const BUILD_OBSERVATION_COMMITMENT_DOMAIN: &[u8] = b"OMEGA-PACKAGE-BUILD-OBSERVATION-COMPARISON\0";

/// Review-only identity of the exact compiler executable bytes observed while
/// evidence was produced. This does not certify the compiler or seal a package
/// instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyCompilerExecutableCommitment([u8; 32]);

impl ReviewOnlyCompilerExecutableCommitment {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }

    pub(crate) const fn from_recovered_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }
}

impl From<CompilerExecutableCommitment> for ReviewOnlyCompilerExecutableCommitment {
    fn from(commitment: CompilerExecutableCommitment) -> Self {
        Self(commitment.digest())
    }
}

/// Review-only identity of the exact package/toolchain source bytes consumed
/// by one compiler run. It is provenance, not admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlySourceConsumptionCommitment([u8; 32]);

impl ReviewOnlySourceConsumptionCommitment {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }

    pub(crate) const fn from_recovered_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }
}

impl From<PackageSourceConsumptionCommitment> for ReviewOnlySourceConsumptionCommitment {
    fn from(commitment: PackageSourceConsumptionCommitment) -> Self {
        Self(commitment.digest())
    }
}

/// Opaque canonical comparison row used by package review orchestration.
///
/// Live rows are copied from an unforgeable compiler-issued review. Recovered
/// rows are constructed only by the compiler's strict recovery-frame decoder
/// and remain distinctly review-only; this type is never compiler evidence or
/// an admission artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyCanonicalRow {
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    key_bytes: Vec<u8>,
    canonical_bytes: Vec<u8>,
    source: PackageReviewCanonicalRowSource,
    recovery_bytes: Option<Vec<u8>>,
}

impl ReviewOnlyCanonicalRow {
    pub const fn kind(&self) -> PackageReviewCanonicalRowKind {
        self.kind
    }

    pub const fn risk(&self) -> PackageReviewCanonicalRowRisk {
        self.risk
    }

    pub fn key_bytes(&self) -> &[u8] {
        &self.key_bytes
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn source(&self) -> &PackageReviewCanonicalRowSource {
        &self.source
    }

    pub(crate) fn from_compiler_issued(row: &PackageReviewCanonicalRow) -> Self {
        Self {
            kind: row.kind(),
            risk: row.risk(),
            key_bytes: row.key_bytes().to_vec(),
            canonical_bytes: row.canonical_bytes().to_vec(),
            source: row.source().clone(),
            recovery_bytes: None,
        }
    }

    pub(crate) fn from_recovered(
        row: &DecodedPackageReviewCanonicalRow,
        recovery_bytes: Vec<u8>,
    ) -> Self {
        Self {
            kind: row.kind(),
            risk: row.risk(),
            key_bytes: row.key_bytes().to_vec(),
            canonical_bytes: row.canonical_bytes().to_vec(),
            source: row.source().clone(),
            recovery_bytes: Some(recovery_bytes),
        }
    }

    pub(crate) fn recovery_bytes(&self) -> Option<&[u8]> {
        self.recovery_bytes.as_deref()
    }
}

/// The package-manager-facing evidence common to a live compiler review and a
/// restart-stable review-only baseline record.
///
/// This trait is deliberately private. Implementing it does not issue accepted
/// evidence or permit construction of a package instance.
pub(crate) trait PackageReviewEvidence {
    fn key(&self) -> &PackageKey;
    fn resolution(&self) -> &ImmutableSourceResolution;
    fn projection_identity_matches(&self) -> bool;
    fn target_name(&self) -> &str;
    fn compiler_executable_commitment(&self) -> ReviewOnlyCompilerExecutableCommitment;
    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment;
    fn build_observation_commitment(&self) -> Option<[u8; 32]>;
    fn whole_review_commitment(&self) -> [u8; 32];
    fn canonical_rows(&self) -> &[ReviewOnlyCanonicalRow];
}

impl PackageReviewEvidence for CompilerIssuedPackageReview {
    fn key(&self) -> &PackageKey {
        CompilerIssuedPackageReview::key(self)
    }

    fn resolution(&self) -> &ImmutableSourceResolution {
        CompilerIssuedPackageReview::resolution(self)
    }

    fn projection_identity_matches(&self) -> bool {
        self.projection().package() == self.key().identity()
    }

    fn target_name(&self) -> &str {
        self.projection().target().target_name()
    }

    fn compiler_executable_commitment(&self) -> ReviewOnlyCompilerExecutableCommitment {
        CompilerIssuedPackageReview::compiler_executable_commitment(self).into()
    }

    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment {
        CompilerIssuedPackageReview::source_consumption_commitment(self).into()
    }

    fn build_observation_commitment(&self) -> Option<[u8; 32]> {
        self.build_observation_summary()
            .map(build_observation_commitment)
    }

    fn whole_review_commitment(&self) -> [u8; 32] {
        whole_review_commitment(self.canonical_review_bytes())
    }

    fn canonical_rows(&self) -> &[ReviewOnlyCanonicalRow] {
        self.comparison_rows()
    }
}

pub(crate) fn whole_review_commitment(canonical_review_bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(WHOLE_REVIEW_COMMITMENT_DOMAIN);
    hash_bytes(&mut digest, canonical_review_bytes);
    digest.finalize().into()
}

pub(crate) fn build_observation_commitment(summary: &BuildObservationSummary) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(BUILD_OBSERVATION_COMMITMENT_DOMAIN);
    digest.update(summary.schema_version().to_le_bytes());
    digest.update([observation_class_tag(summary.ceiling())]);
    digest.update([observation_class_tag(summary.realized())]);
    digest.update(summary.filesystem_operation_schema_version().to_le_bytes());
    digest.update(
        u64::try_from(summary.filesystem_operation_attempts().len())
            .expect("build observation attempt count fits u64")
            .to_le_bytes(),
    );
    for attempt in summary.filesystem_operation_attempts() {
        digest.update(attempt.operation_tag().to_le_bytes());
        digest.update([filesystem_provider_tag(attempt.provider())]);
        digest.update(attempt.result().to_le_bytes());
        digest.update(attempt.post_error().to_le_bytes());
        digest.update(
            u64::try_from(attempt.grant_refusals().len())
                .expect("build observation refusal count fits u64")
                .to_le_bytes(),
        );
        for refusal in attempt.grant_refusals() {
            digest.update([refusal.operand_ordinal()]);
            digest.update([grant_access_tag(refusal.access())]);
            digest.update([grant_refusal_reason_tag(refusal.reason())]);
        }
    }
    digest.finalize().into()
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("review evidence byte length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

const fn observation_class_tag(class: BuildObservationClass) -> u8 {
    match class {
        BuildObservationClass::Hermetic => 0,
        BuildObservationClass::Receipted => 1,
        BuildObservationClass::Volatile => 2,
    }
}

const fn filesystem_provider_tag(provider: BuildFilesystemProvider) -> u8 {
    match provider {
        BuildFilesystemProvider::Virtual => 0,
        BuildFilesystemProvider::RealUnscoped => 1,
        BuildFilesystemProvider::RealScoped => 2,
    }
}

const fn grant_access_tag(access: BuildFilesystemGrantAccess) -> u8 {
    match access {
        BuildFilesystemGrantAccess::Read => 0,
        BuildFilesystemGrantAccess::Write => 1,
    }
}

const fn grant_refusal_reason_tag(reason: BuildFilesystemGrantRefusalReason) -> u8 {
    match reason {
        BuildFilesystemGrantRefusalReason::Unresolvable => 0,
        BuildFilesystemGrantRefusalReason::OutsideGrantedRoots => 1,
    }
}
