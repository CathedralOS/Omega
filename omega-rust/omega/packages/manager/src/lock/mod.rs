//! Persistent project policy, separate from fresh compiler authorization.
//!
//! Exact target sections retain the immutable source graph, complete normalized
//! policy baselines, and source-bound historical decisions. Recovery validates
//! records without old source or compiler execution; it does not publish files
//! or turn recorded choices into authorization for a changed candidate.

mod decisions;
mod error;
mod limits;
mod model;
mod text;

pub use error::PackageLockError;
pub use limits::PackageLockRecoveryLimits;
pub use model::{PackageLock, PackageLockTarget};
pub const PACKAGE_LOCK_VERSION: u16 = 1;

pub use decisions::{
    HistoricalPackagePolicyDecision, HistoricalPackagePolicyDecisions,
    HistoricalPackagePolicyError, HistoricalPackagePolicyLimits,
    HistoricalPackagePolicyRecoveryUsage,
};
