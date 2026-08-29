//! Target-neutral semantic analyses and their catalog-facing compute joins.
//!
//! Each leaf owns one independently cached fact family. The shared leaf owns
//! only immutable scalar-CFG projections used by more than one analysis.

mod effect_summaries;
mod ownership_frontiers;
mod shared;
mod sparse_conditional_constants;
mod use_definitions;
mod value_liveness;
mod value_ranges;

pub use effect_summaries::{
    EffectClass, EffectKnowledge, EffectSummaryAnalysis, FunctionEffectSummary, NodeEffectSummary,
};
pub use ownership_frontiers::{OwnershipFrontierAnalysis, OwnershipFrontierAnalysisFact};
pub use sparse_conditional_constants::{
    ExecutableEdgeAnalysis, ExecutableEdgeFact, ExecutableEdgeKnowledge, ScalarConstant,
    ScalarConstantAnalysis, ScalarConstantFact, ScalarConstantSupport, ValueFactRegion,
};
pub use use_definitions::UseDefinitionAnalysis;
pub use value_liveness::{NodeLiveness, ValueLivenessAnalysis, ValueLivenessBlock};
pub use value_ranges::ValueRangeAnalysis;

pub(super) use effect_summaries::effect_summaries;
pub(super) use ownership_frontiers::ownership_frontiers;
pub(super) use sparse_conditional_constants::{executable_edges, scalar_constants};
pub(super) use use_definitions::use_definitions;
pub(super) use value_liveness::value_liveness;
pub(super) use value_ranges::value_ranges;
