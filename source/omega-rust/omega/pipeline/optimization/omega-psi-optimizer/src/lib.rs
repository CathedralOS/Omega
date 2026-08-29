#![forbid(unsafe_code)]

//! Deterministic target-neutral analyses and rewrite orchestration for verified
//! Psi optimization units.
//!
//! This crate is not constructed by the ordinary empty-selection compiler
//! path. Callers must explicitly enter the verified optimizer pipeline before
//! creating an [`AnalysisManager`].

mod analyses;
mod pass_manager;
mod registry;
mod rules;

pub use analyses::{
    AnalysisManager, AnalysisManagerError, AnalysisProduct, AnalysisRevisionCommit,
    BlockControlFlow, CallGraphAnalysis, ControlFlowAnalysis, DominatorAnalysis, EffectClass,
    EffectKnowledge, EffectSummaryAnalysis, ExecutableEdgeAnalysis, ExecutableEdgeFact,
    ExecutableEdgeKnowledge, ExitKind, FunctionControlFlow, FunctionEffectSummary, LoopAnalysis,
    LoopRegion, NodeEffectSummary, NodeLiveness, OwnershipFrontierAnalysis,
    OwnershipFrontierAnalysisFact, ScalarConstant, ScalarConstantAnalysis, ScalarConstantFact,
    ScalarConstantSupport, StronglyConnectedComponentAnalysis, UseDefinitionAnalysis,
    ValueFactRegion, ValueLivenessAnalysis, ValueLivenessBlock, ValueRangeAnalysis, ValueRangeFact,
    ValueRangeRegion, ValueRangeScope, ValueRangeSupport, analysis_dependencies, compute_analysis,
    value_range_fact_identity,
};
pub use pass_manager::{
    CandidateContractAxis, ExternalDecisionContextAxis, ExternalDecisionReplayError,
    OptimizationRun, OptimizationRunError, OptimizationRunUsage, PsiOptimizationCommit,
    VerifiedPsiOptimizationSession, baseline_psi_cost_model_identity, replay_psi_pipeline,
    replay_psi_registry, run_psi_pipeline, run_psi_registry, validate_external_decision_recording,
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
    WrappingIntegerSubtractConstantsRule, built_in_psi_registries, built_in_psi_registry,
};
