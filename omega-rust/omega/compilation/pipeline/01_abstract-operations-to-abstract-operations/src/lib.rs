#![forbid(unsafe_code)]

//! Optimizer module role: crate map.
//! Post-Terminal abstract-operation optimization over units reconstructed from
//! Terminal Psi. This does not implement the portable pre-Terminal Psi phase.
//! Empty selection is identity and precedes `AnalysisManager` construction.
//!
//! Start at [`optimize_abstract_operations`] in `abstract_optimization.rs`: it
//! admits the artifact, builds the verified unit, runs the selected passes
//! through `pass_manager` over the rules that `rules::registry` orders, and
//! publishes the validated plan through `publication`.
//!
//! Only that entry, its [`ValidatedOptimizedAbstractPlan`], the three error
//! types and [`validation`] are public. The ranked rewrites run in every
//! build: loop-invariant scalar motion is a scheduled pass step the selected
//! pipeline can reach, while the countdown constant relocation boundary and
//! the loop analyses it consumes stay available to scheduled steps and to
//! harnesses through `test_support`, which names what the
//! `psi_optimization_allocations` example drives directly.
/// `Display` for the crate's `<context> failed: {self:?}` error convention.
macro_rules! failed_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }
    };
}

mod abstract_optimization;
mod analyses;
pub(crate) use optimization_core::codec_bytes;
mod field_value_specialization;
mod pass_manager;
mod publication;
mod ranked_rewrites;
mod representation_specialization;
mod rules;
mod state_specialization;
pub mod validation;

pub use abstract_optimization::{AbstractOptimizationError, optimize_abstract_operations};
pub(crate) use publication::OptimizedAbstractProjectionError;
pub use publication::ValidatedOptimizedAbstractPlan;

pub(crate) use analyses::{AnalysisManager, AnalysisProduct, compute_analysis};
pub(crate) use analyses::{
    AnalysisManagerError, CallGraphAnalysis, EffectClass, EffectKnowledge, EffectSummaryAnalysis,
    OwnershipFrontierAnalysis, ScalarConstantAnalysis, UseDefinitionAnalysis, ValueRangeAnalysis,
};
pub(crate) use analyses::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantPlacementAnalysisError,
    CountedLoopAnalysisError, ValidatedCountdownInvariantConstantAnalysis,
    ValidatedCountdownInvariantConstantPlacementAnalysis, ValidatedCountedLoopAnalysis,
    analyze_countdown_invariant_constant_placement, analyze_countdown_invariant_constants,
    analyze_counted_loops,
};
pub(crate) use optimization_unit::ScalarConstantValue;
pub(crate) use pass_manager::PsiValidatedCandidateDeclaration;
pub(crate) use pass_manager::VerifiedPsiOptimizationSession;
pub(crate) use pass_manager::{OptimizationRun, OptimizationRunUsage, PsiOptimizationCommit};
pub(crate) use rules::registry::{
    OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    RuleRegistryError,
};

/// The analyses, ranked rewrites, rule catalog, and pipeline runs exposed to
/// harnesses outside the crate, such as the `psi_optimization_allocations`
/// example. Production consumers use only the stage entry and its validated
/// plan above.
#[cfg(feature = "test-support")]
pub mod test_support {
    pub use crate::abstract_optimization::run_psi_pipeline;

    pub use crate::analyses::{
        AnalysisRevision, ValueRangeFact, ValueRangeScope, ValueRangeSupport,
        value_range_fact_identity,
    };

    pub use crate::pass_manager::{OptimizationRunUsage, PsiOptimizationCommit};

    pub use crate::rules::WrappingIntegerAddConstantsRule;
}
