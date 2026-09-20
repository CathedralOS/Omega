//! Persistent project policy, separate from fresh compiler authorization.
//!
//! Exact target sections retain the immutable source graph, compact exact
//! acceptance rows, and source-bound historical decisions. Recovery validates
//! records without old source or compiler execution; it does not publish files
//! or turn recorded choices into authorization for a changed candidate.

mod acceptance;
pub use acceptance::{PackageAcceptanceRow, PackagePolicyAcceptance, PackagePolicyOccurrence};
mod decisions;
mod error;
mod limits;
mod model;
pub(crate) mod occurrences;
mod text;
mod validation;

pub use error::PackageLockError;
pub use limits::PackageLockRecoveryLimits;
pub use model::{PackageLock, PackageLockTarget};
pub use occurrences::{
    PackageCheckedContext, PackageOccurrenceRoster, PackageOccurrenceRosterError,
    PackagePurposeCoverage,
};
pub const PACKAGE_LOCK_VERSION: u16 = 3;

pub use decisions::{
    HistoricalPackagePolicyDecision, HistoricalPackagePolicyDecisionSubject,
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyError, HistoricalPackagePolicyLimits,
    HistoricalPackagePolicyRecoveryUsage,
};
