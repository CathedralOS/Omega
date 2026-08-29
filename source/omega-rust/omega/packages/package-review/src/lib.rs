#![forbid(unsafe_code)]

//! Compiler-issued package review evidence.
//!
//! The crate root is intentionally only the public entrance: stable review
//! vocabulary lives in [`evidence`], compiler-to-review conversion in
//! [`projection`], canonical persistence/recovery in [`encoding`], and local
//! reconstruction questions in [`obligation_ledger`]. This is a review surface,
//! not accepted package admission evidence.

mod encoding;
mod evidence;
mod obligation_ledger;
mod projection;

pub use encoding::{
    DecodedPackageReviewCanonicalRow, PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION,
    PACKAGE_REVIEW_ENCODING_VERSION, PACKAGE_REVIEW_ROW_ENCODING_VERSION,
    PackageReviewCanonicalRowRecoveryError, PackageReviewCanonicalRowRecoveryLimits,
    PackageReviewEncodingError, decode_package_review_canonical_row,
    decode_package_review_canonical_row_with_limits, encode_package_review_canonical_row,
    encode_package_review_canonical_row_with_limits,
};
pub use evidence::*;
pub use obligation_ledger::{
    ORDINARY_PACKAGE_OBLIGATION_LEDGER_ENCODING_VERSION,
    ORDINARY_PACKAGE_OBLIGATION_SCHEMA_VERSION, OrdinaryPackageObligationLedger,
    OrdinaryPackageObligationLedgerFingerprint, OrdinaryPackageObligationLedgerRecoveryError,
    OrdinaryPackageObligationRow, OrdinaryPackageObligationSchemaIdentity,
    decode_ordinary_package_obligation_ledger, encode_ordinary_package_obligation_ledger,
    ordinary_package_obligation_ledger_fingerprint,
    ordinary_package_obligation_ledger_from_compiler_rows,
    reconstruct_ordinary_package_obligation_ledger, recover_ordinary_package_obligation_ledger,
    validate_ordinary_package_obligation_ledger,
};
pub use projection::project_checked_package_review;
