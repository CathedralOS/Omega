//! Persistent project policy, separate from fresh compiler authorization.
//!
//! Historical decisions are one section of the pending accepted-lock format.
//! The enclosing lock must also retain the complete source subject and the
//! normalized accepted API, capability, and assumption baseline. Neither a
//! decision fingerprint nor this section alone supplies that baseline.

mod decisions;

pub use decisions::{
    HistoricalPackagePolicyDecision, HistoricalPackagePolicyDecisions,
    HistoricalPackagePolicyError, HistoricalPackagePolicyLimits,
};
