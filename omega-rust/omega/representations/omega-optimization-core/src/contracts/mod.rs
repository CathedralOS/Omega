//! Optimizer module role: stage group. Stable declarations governing analyses, decisions, work, and rules.
//!
//! `analysis` owns required and invalidated fact sets. `decision` owns safety
//! and candidate dispositions. `budget` owns bounded work. `rule` binds those
//! families to exact pass and rule identities. All wire decoders share the
//! closed failures in `error`.

mod analysis;
mod budget;
mod decision;
mod error;
mod rule;

pub use analysis::{AnalysisInvalidationSet, AnalysisKind, AnalysisSet};
pub use budget::{InvalidOptimizationWorkBudget, OptimizationWorkBudget};
pub use decision::{OptimizationCandidateVerdict, OptimizationReasonCode, OptimizationSafetyClass};
pub use error::CoreContractDecodeError;
pub use rule::{InvalidOptimizationRuleContract, OptimizationRuleContract};

#[cfg(test)]
mod tests;
