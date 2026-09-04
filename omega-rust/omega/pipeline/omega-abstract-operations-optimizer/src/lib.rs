#![forbid(unsafe_code)]

//! Optimizer module role: crate map.
//! Post-Terminal abstract-operation optimization over units reconstructed from
//! Terminal Psi. This does not implement the portable pre-Terminal Psi phase.
//! Empty selection is identity and precedes [`AnalysisManager`] construction.
mod analyses;
mod pass_manager;
mod ranked_rewrites;
mod registry;
mod rules;

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
pub use pass_manager::{
    CandidateContractAxis, ExternalDecisionContextAxis, ExternalDecisionReplayError,
    OptimizationRun, OptimizationRunError, OptimizationRunUsage, PsiOptimizationCommit,
    PsiValidatedCandidateDeclaration, VerifiedPsiOptimizationSession,
    baseline_psi_cost_model_identity, replay_psi_pipeline, replay_psi_registry, run_psi_pipeline,
    run_psi_pipeline_for_projection, run_psi_registry, validate_external_decision_recording,
};
pub use ranked_rewrites::{
    AppliedCountdownInvariantConstantRelocation, CountdownInvariantConstantRelocation,
    CountdownInvariantConstantRelocationCandidate, CountdownInvariantConstantRelocationError,
    ValidatedCountdownInvariantConstantRelocation, apply_countdown_invariant_constant_relocation,
    propose_countdown_invariant_constant_relocations,
    validate_countdown_invariant_constant_relocation,
};
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
