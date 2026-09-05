//! Exact project decisions over complete normalized policy deltas.
//!
//! These values resolve policy blockers, not compiler obligations, source
//! custody, audit completion, admission, or publication.

mod error;
mod limits;
mod model;
mod obligations;
mod resolution;
mod text;

pub use error::PackagePolicyDecisionError;
pub use limits::PackagePolicyDecisionLimits;
pub use model::{
    PackagePolicyDecision, PackagePolicyDecisionObligation, PackagePolicyDecisionResolution,
    PackagePolicyDecisionResolutionFingerprint, PackagePolicyDecisionSubject,
    PackagePolicyObligationFingerprint,
};
pub use resolution::resolve_package_policy_decisions;
pub use text::recover_package_policy_decisions;
