//! Source-scoped historical decisions and their bounded text representation.

mod capture;
mod model;
mod text;

pub use model::{
    HistoricalPackagePolicyDecision, HistoricalPackagePolicyDecisions,
    HistoricalPackagePolicyError, HistoricalPackagePolicyLimits,
    HistoricalPackagePolicyRecoveryUsage,
};
