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
mod abstract_optimization;
mod analyses;
mod field_value_specialization;
mod pass_manager;
mod publication;
mod ranked_rewrites;
mod representation_specialization;
mod rules;
mod state_specialization;
pub mod validation;

pub use abstract_optimization::{AbstractOptimizationError, optimize_abstract_operations};
#[cfg(test)]
pub(crate) use abstract_optimization::{replay_psi_pipeline, run_psi_pipeline, run_psi_registry};
#[cfg(test)]
pub(crate) use publication::publish_optimization_run;
pub use publication::{OptimizedAbstractProjectionError, ValidatedOptimizedAbstractPlan};

pub(crate) use analyses::{AnalysisManager, AnalysisProduct, ScalarConstant, compute_analysis};
pub(crate) use analyses::{
    AnalysisManagerError, CallGraphAnalysis, EffectClass, EffectKnowledge, EffectSummaryAnalysis,
    OwnershipFrontierAnalysis, ScalarConstantAnalysis, StronglyConnectedComponentAnalysis,
    UseDefinitionAnalysis, ValueRangeAnalysis,
};
pub(crate) use analyses::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantPlacementAnalysisError,
    CountedLoopAnalysisError, ValidatedCountdownInvariantConstantAnalysis,
    ValidatedCountdownInvariantConstantPlacementAnalysis, ValidatedCountedLoopAnalysis,
    analyze_countdown_invariant_constant_placement, analyze_countdown_invariant_constants,
    analyze_counted_loops,
};
#[cfg(any(test, feature = "test-support"))]
pub(crate) use analyses::{
    CountdownInvariantConstantAnalysisSnapshot,
    CountdownInvariantConstantPlacementAnalysisSnapshot, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, CountedLoopAnalysisSnapshot,
    UnsignedCountdownInvariantConstantPlacements, validate_countdown_invariant_constant_analysis,
    validate_countdown_invariant_constant_placement_analysis, validate_counted_loop_analysis,
};
pub use pass_manager::OptimizationRunError;
pub(crate) use pass_manager::PsiValidatedCandidateDeclaration;
pub(crate) use pass_manager::VerifiedPsiOptimizationSession;
pub(crate) use pass_manager::{
    OptimizationRun, OptimizationRunUsage, PsiOptimizationCommit, baseline_psi_cost_model_identity,
};
pub use ranked_rewrites::LoopInvariantScalarMotionError;
#[cfg(test)]
pub(crate) use ranked_rewrites::propose_countdown_invariant_constant_relocations;
#[cfg(test)]
pub(crate) use ranked_rewrites::{
    apply_loop_invariant_scalar_motion, propose_loop_invariant_scalar_motion,
    validate_loop_invariant_scalar_motion,
};
pub(crate) use rules::PsiPassStep;
pub(crate) use rules::registry::{
    OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    RuleRegistryError,
};
#[cfg(test)]
pub(crate) use rules::{WrappingIntegerAddConstantsRule, built_in_psi_registry};

/// The analyses, ranked rewrites, rule catalog, and pipeline runs exposed to
/// harnesses outside the crate, such as the `psi_optimization_allocations`
/// example. Production consumers use only the stage entry and its validated
/// plan above.
#[cfg(feature = "test-support")]
pub mod test_support {
    pub use crate::abstract_optimization::{
        replay_psi_pipeline, run_psi_pipeline, run_psi_registry,
    };
    pub use crate::analyses::{
        AnalysisManager, AnalysisProduct, AnalysisRevision, ScalarConstant, ValueRangeFact,
        ValueRangeScope, ValueRangeSupport, compute_analysis, value_range_fact_identity,
    };
    pub use crate::analyses::{
        CountdownInvariantConstantAnalysisError, CountdownInvariantConstantAnalysisSnapshot,
        CountdownInvariantConstantPlacementAnalysisError,
        CountdownInvariantConstantPlacementAnalysisSnapshot, CountdownInvariantConstantRole,
        CountedLoopAnalysisError, CountedLoopAnalysisSnapshot,
    };
    pub use crate::pass_manager::{
        OptimizationRun, OptimizationRunUsage, PsiOptimizationCommit,
        VerifiedPsiOptimizationSession, baseline_psi_cost_model_identity,
    };
    pub use crate::publication::{AppliedDecisionCustodyAxis, publish_optimization_run};
    pub use crate::ranked_rewrites::{
        CountdownInvariantConstantRelocationError, LoopInvariantScalarMotionError,
        apply_countdown_invariant_constant_relocation, apply_loop_invariant_scalar_motion,
        propose_countdown_invariant_constant_relocations, propose_loop_invariant_scalar_motion,
        validate_countdown_invariant_constant_relocation, validate_loop_invariant_scalar_motion,
    };
    pub use crate::rules::{
        PSI_PASS_CATALOG, WrappingIntegerAddConstantsRule, built_in_psi_registry,
    };
}
