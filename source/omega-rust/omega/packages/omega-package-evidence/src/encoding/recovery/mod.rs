//! Bounded recovery of canonical rows and their persisted source envelopes.
//!
//! This owner validates canonical row framing for local reconstruction and
//! owns both directions of the public recovered-row envelope. It does not
//! produce package-review comparison rows; that belongs to `encode`.

mod codec;
mod envelope;
mod framing;
mod model;
mod source;

use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

pub use envelope::{
    decode_package_review_canonical_row, decode_package_review_canonical_row_with_limits,
    encode_package_review_canonical_row, encode_package_review_canonical_row_with_limits,
};
pub use model::{
    DecodedPackageReviewCanonicalRow, PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION,
    PackageReviewCanonicalRowRecoveryError, PackageReviewCanonicalRowRecoveryLimits,
};

use crate::evidence::{PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk};

pub(crate) fn canonical_row_subject_for_ledger(
    canonical_bytes: &[u8],
) -> Result<(PackageKeyIdentity, TargetProfile), PackageReviewCanonicalRowRecoveryError> {
    let framing = canonical_row_framing_for_ledger(canonical_bytes)?;
    Ok((framing.package, framing.target))
}

pub(crate) fn canonical_row_framing_for_ledger(
    canonical_bytes: &[u8],
) -> Result<CanonicalRowLedgerFraming, PackageReviewCanonicalRowRecoveryError> {
    let framing = framing::parse_canonical_row(
        canonical_bytes,
        PackageReviewCanonicalRowRecoveryLimits::default(),
    )?;
    Ok(CanonicalRowLedgerFraming {
        package: framing.package,
        target: framing.target,
        kind: framing.kind,
        risk: framing.risk,
        key_bytes: framing.key_bytes,
    })
}

pub(crate) struct CanonicalRowLedgerFraming {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: TargetProfile,
    pub(crate) kind: PackageReviewCanonicalRowKind,
    pub(crate) risk: PackageReviewCanonicalRowRisk,
    pub(crate) key_bytes: Vec<u8>,
}
