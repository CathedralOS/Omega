//! Source-scoped historical decisions and their bounded text representation.

mod capture;
mod model;
mod normalized;
mod text;

pub use model::{
    HistoricalPackagePolicyDecision, HistoricalPackagePolicyDecisionSubject,
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyError, HistoricalPackagePolicyLimits,
    HistoricalPackagePolicyRecoveryUsage,
};
