// Optimization rule, analysis, work-budget, and verdict contracts.
//
// `analysis` defines the closed 21-bit `AnalysisKind` vocabulary and bitsets
// (`AnalysisSet`, `AnalysisInvalidationSet`); `budget` defines the 5-axis
// `OptimizationWorkBudget` scaled per `OPTIMIZATION_REFERENCE_EXTENT`;
// `decision` defines `OptimizationSafetyClass`, `OptimizationReasonCode`, and
// `OptimizationCandidateVerdict`; and `rule` binds them into the 97-byte
// `OptimizationRuleContract` (`OMGRUL\0\0` v1).

mod analysis;
mod budget;
mod decision;
mod error;
mod rule;

pub use analysis::{AnalysisInvalidationSet, AnalysisKind, AnalysisSet};
pub use budget::{OPTIMIZATION_REFERENCE_EXTENT, OptimizationWorkBudget};
pub use decision::{OptimizationCandidateVerdict, OptimizationReasonCode, OptimizationSafetyClass};
pub(crate) use error::CoreContractDecodeError;
pub use rule::OptimizationRuleContract;

#[cfg(test)]
mod tests;
