#![forbid(unsafe_code)]

//! Optimizer module role: crate map.
//! Post-Terminal abstract-operation optimization over units reconstructed from
//! Terminal Psi. This does not implement the portable pre-Terminal Psi phase.
//! Empty selection is identity and precedes [`AnalysisManager`] construction.
//!
//! Start at [`optimize_abstract_operations`] in `abstract_optimization.rs`: it
//! admits the artifact, builds the verified unit, runs the selected passes
//! through [`pass_manager`] over the rules that `rules::registry` orders, and
//! publishes the validated plan through [`publication`].
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

pub use abstract_optimization::{
    AbstractOptimizationError, optimize_abstract_operations, replay_psi_pipeline, run_psi_pipeline,
    run_psi_registry,
};
pub use publication::{
    AppliedDecisionCustodyAxis, OptimizedAbstractProjectionError, ValidatedOptimizedAbstractPlan,
    publish_optimization_run,
};

pub use analyses::{
    AnalysisManager, AnalysisManagerError, AnalysisProduct, AnalysisRevision,
    AnalysisRevisionCommit, BlockControlFlow, CallGraphAnalysis, ControlFlowAnalysis,
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantAnalysisSnapshot,
    CountdownInvariantConstantConsumer, CountdownInvariantConstantDestination,
    CountdownInvariantConstantPlacement, CountdownInvariantConstantPlacementAnalysisError,
    CountdownInvariantConstantPlacementAnalysisSnapshot, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, CountedLoopAnalysisError, CountedLoopAnalysisSnapshot,
    DominatorAnalysis, EffectClass, EffectKnowledge, EffectSummaryAnalysis, ExactUnsignedTripCount,
    ExecutableEdgeAnalysis, ExecutableEdgeFact, ExecutableEdgeKnowledge, ExitKind,
    FunctionControlFlow, FunctionEffectSummary, LoopAnalysis, LoopRegion, NodeEffectSummary,
    NodeLiveness, OwnershipFrontierAnalysis, OwnershipFrontierAnalysisFact, PlaceAliasClaim,
    PlaceAliasFunction, PlaceAliasRelation, PlaceAliasRoot, PlaceAliasesAnalysis, PlaceView,
    ScalarConstant, ScalarConstantAnalysis, ScalarConstantFact, ScalarConstantSupport,
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
pub use field_value_specialization::{
    AppliedFieldValueSpecialization, FieldValueSpecializationCandidate,
    FieldValueSpecializationError, ResolvedFieldValue, ValidatedFieldValueSpecialization,
    apply_field_value_specialization, validate_field_value_specialization,
};
// Proposal helpers have no consumer outside this crate; they are internal
// plumbing, not stage entrances.
pub(crate) use field_value_specialization::propose_field_value_specializations;
pub use pass_manager::{
    CandidateContractAxis, ExternalDecisionContextAxis, ExternalDecisionReplayError,
    OptimizationRun, OptimizationRunError, OptimizationRunUsage, PsiOptimizationCommit,
    PsiValidatedCandidateDeclaration, VerifiedPsiOptimizationSession,
    baseline_psi_cost_model_identity, validate_external_decision_recording,
};
pub use ranked_rewrites::{
    AppliedCountdownInvariantConstantRelocation, AppliedLoopInvariantScalarMotion,
    CountdownInvariantConstantRelocation, CountdownInvariantConstantRelocationCandidate,
    CountdownInvariantConstantRelocationError, LoopInvariantNodeResult,
    LoopInvariantScalarMotionCandidate, LoopInvariantScalarMotionError, LoopInvariantScalarNode,
    LoopInvariantScalarRelocation, ValidatedCountdownInvariantConstantRelocation,
    ValidatedLoopInvariantScalarMotion, apply_countdown_invariant_constant_relocation,
    apply_loop_invariant_scalar_motion, propose_countdown_invariant_constant_relocations,
    propose_loop_invariant_scalar_motion, validate_countdown_invariant_constant_relocation,
    validate_loop_invariant_scalar_motion,
};
pub(crate) use representation_specialization::propose_case_membership_specializations;
pub use representation_specialization::{
    AppliedCaseMembershipSpecialization, CaseMembershipSpecializationCandidate,
    CaseMembershipSpecializationError, ResolvedCaseMembership,
    ValidatedCaseMembershipSpecialization, apply_case_membership_specialization,
    validate_case_membership_specialization,
};
pub use rules::registry::{
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
pub use state_specialization::{
    AppliedStateArgumentSpecialization, SpecializedStateEdge, StateArgumentSpecializationCandidate,
    StateArgumentSpecializationError, ValidatedStateArgumentSpecialization,
    apply_state_argument_specialization,
};
pub(crate) use state_specialization::{
    propose_state_argument_specializations, validate_state_argument_specialization,
};
