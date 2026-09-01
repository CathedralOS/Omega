//! Local reconstruction ledger for ordinary package-review obligations.
//!
//! This mirrors the Terminal obligation-ledger rule at the ordinary package
//! layer: recovered producer rows are inert until the selected local compiler
//! reconstructs the complete current row set from compiler-owned semantic state
//! after successful checking and requires exact equality. The current row
//! vocabulary remains review-only and incomplete for accepted `PackageInstance`
//! evidence; this module does not promote it into a lock or certificate.

mod codec;
mod construction;
mod limits;
mod model;
mod results;
mod validation;

pub use codec::{
    decode_ordinary_package_obligation_ledger, encode_ordinary_package_obligation_ledger,
    ordinary_package_obligation_ledger_fingerprint,
};
pub use construction::{
    ordinary_package_obligation_ledger_from_compiler_rows,
    recover_ordinary_package_obligation_ledger,
};
pub use limits::{
    ORDINARY_PACKAGE_OBLIGATION_LEDGER_ENCODING_VERSION, ORDINARY_PACKAGE_OBLIGATION_SCHEMA_VERSION,
};
pub use model::{
    OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerFingerprint,
    OrdinaryPackageObligationLedgerRecoveryError, OrdinaryPackageObligationRow,
    OrdinaryPackageObligationSchemaIdentity,
};
pub use results::{
    OrdinaryPackageAcceptedClaimObligation, OrdinaryPackageDangerousAuthorityObligation,
    OrdinaryPackageExternalExecutableSupplyObligation, OrdinaryPackageObligationResultSet,
    OrdinaryPackageObligationStatus, ordinary_package_obligation_results_from_projection,
    reconstruct_ordinary_package_obligation_results, validate_ordinary_package_obligation_results,
};
pub use validation::{
    reconstruct_ordinary_package_obligation_ledger, validate_ordinary_package_obligation_ledger,
};
