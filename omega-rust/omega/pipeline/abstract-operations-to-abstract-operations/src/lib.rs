#![forbid(unsafe_code)]

//! Optimizer module role: crate map.
//! Post-Terminal abstract-operation optimization over units reconstructed from
//! Terminal Psi. This does not implement the portable pre-Terminal Psi phase.
//! Empty selection is identity and precedes [`AnalysisManager`] construction.
mod analyses;
mod pass_manager;
mod phase;
mod publication;
mod ranked_rewrites;
mod registry;
mod rules;
pub mod validation;

pub use phase::{AbstractOptimizationError, optimize_abstract_operations};
pub use publication::{
    AppliedDecisionCustodyAxis, OptimizedAbstractProjectionError, ValidatedOptimizedAbstractPlan,
    publish_optimization_run,
};

pub use analyses::{
    AnalysisManager, AnalysisManagerError, AnalysisProduct, AnalysisRevisionCommit,
    BlockControlFlow, CallGraphAnalysis, ControlFlowAnalysis,
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantAnalysisSnapshot,
    CountdownInvariantConstantConsumer, CountdownInvariantConstantDestination,
    CountdownInvariantConstantPlacement, CountdownInvariantConstantPlacementAnalysisError,
    CountdownInvariantConstantPlacementAnalysisSnapshot, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, CountedLoopAnalysisError, CountedLoopAnalysisSnapshot,
    DominatorAnalysis, EffectClass, EffectKnowledge, EffectSummaryAnalysis, ExactUnsignedTripCount,
    ExecutableEdgeAnalysis, ExecutableEdgeFact, ExecutableEdgeKnowledge, ExitKind,
    FunctionControlFlow, FunctionEffectSummary, LoopAnalysis, LoopRegion, NodeEffectSummary,
    NodeLiveness, OwnershipFrontierAnalysis, OwnershipFrontierAnalysisFact, ScalarConstant,
    ScalarConstantAnalysis, ScalarConstantFact, ScalarConstantSupport,
    StronglyConnectedComponentAnalysis, UnsignedCountdownInvariantConstantPlacements,
    UnsignedCountdownInvariantConstants, UnsignedCountdownLoopSummary, UseDefinitionAnalysis,
    ValidatedCountdownInvariantConstantAnalysis,
    ValidatedCountdownInvariantConstantPlacementAnalysis, ValidatedCountedLoopAnalysis,
    ValueFactRegion, ValueLivenessAnalysis, ValueLivenessBlock, ValueRangeAnalysis, ValueRangeFact,
    ValueRangeRegion, ValueRangeScope, ValueRangeSupport, analysis_dependencies, compute_analysis,
    value_range_fact_identity,
};
pub(crate) use analyses::{
    analyze_countdown_invariant_constant_placement, analyze_countdown_invariant_constants,
    analyze_counted_loops, validate_countdown_invariant_constant_analysis,
    validate_countdown_invariant_constant_placement_analysis, validate_counted_loop_analysis,
};
pub use pass_manager::*;
pub use ranked_rewrites::*;
pub use registry::{
    OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    RuleRegistryError, RuleScheduleKey,
};
pub use rules::{
    AdjacentBlockMergeRule, BooleanEqualConstantsRule, BooleanNotConstantsRule,
    ConstantConditionalFoldRule, DeadScalarLiteralEliminationRule,
    DeadUnconditionallyTotalScalarEliminationRule,
    DominatorProofCertifiedCompatiblePolicyScalarGvnRule, DominatorProofCertifiedScalarGvnRule,
    DominatorTotalScalarGvnRule, ExactIntegerAddConstantsRule, ExactIntegerCastConstantsRule,
    ExactIntegerDivideConstantsRule, ExactIntegerMultiplyConstantsRule,
    ExactIntegerRemainderConstantsRule, ExactIntegerShiftLeftConstantsRule,
    ExactIntegerShiftRightConstantsRule, ExactIntegerSubtractConstantsRule,
    IntegerBitwiseAndConstantsRule, IntegerBitwiseNotConstantsRule, IntegerBitwiseOrConstantsRule,
    IntegerBitwiseXorConstantsRule, IntegerEqualConstantRangeRule, IntegerEqualConstantsRule,
    IntegerEqualRangeConstantRule, IntegerEqualRangeRangeRule, IntegerLessOrEqualConstantRangeRule,
    IntegerLessOrEqualConstantsRule, IntegerLessOrEqualRangeConstantRule,
    IntegerLessOrEqualRangeRangeRule, IntegerLessThanConstantRangeRule,
    IntegerLessThanConstantsRule, IntegerLessThanRangeConstantRule, IntegerLessThanRangeRangeRule,
    IntegerWidenConstantsRule, LinearEmptyBlockThreadRule,
    LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule,
    LiveProofCertifiedExactIntegerSelfSubtractEliminationRule,
    LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule,
    LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule,
    LiveProofCertifiedIntegerDivideByOneEliminationRule,
    LiveProofCertifiedIntegerIdentityEliminationRule,
    LiveProofCertifiedIntegerRemainderByOneEliminationRule,
    LiveProofCertifiedIntegerSelfDivideEliminationRule,
    LiveProofCertifiedIntegerSelfRemainderEliminationRule,
    LiveProofCertifiedIntegerZeroDividendEliminationRule,
    LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule,
    NonAdjacentBlockMergeRule, ORDERED_PSI_PASSES, PSI_PASS_CATALOG,
    PhiTranslatedObligationFreeScalarGvnRule,
    PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule,
    PhiTranslatedProofCertifiedScalarGvnRule, ProofCertifiedDeadScalarEliminationRule,
    PsiPassCatalogEntry, PsiPassTargetApplicability, RedundantBlockParameterRule,
    SameBlockProofCertifiedCompatiblePolicyScalarCseRule, SameBlockProofCertifiedScalarCseRule,
    SameBlockTotalScalarCseRule, SaturatingIntegerAddConstantsRule,
    SaturatingIntegerDivideConstantsRule, SaturatingIntegerMultiplyConstantsRule,
    SaturatingIntegerRemainderConstantsRule, SaturatingIntegerSubtractConstantsRule,
    SharedJumpFusionRule, WrappingIntegerAddConstantsRule, WrappingIntegerDivideConstantsRule,
    WrappingIntegerMultiplyConstantsRule, WrappingIntegerRemainderConstantsRule,
    WrappingIntegerShiftLeftConstantsRule, WrappingIntegerShiftRightConstantsRule,
    WrappingIntegerSubtractConstantsRule, built_in_psi_registries,
    built_in_psi_registries_for_selections, built_in_psi_registry,
    built_in_psi_registry_for_selections,
};
