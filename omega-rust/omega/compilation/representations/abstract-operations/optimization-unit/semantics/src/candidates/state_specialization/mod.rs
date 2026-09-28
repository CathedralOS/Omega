//! Optimizer module role: executable entrance. State-argument specialization validation.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite, validate_psi_optimization_unit};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleIdentity,
    OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod replay;

pub(crate) use replay::cyclic_machines;

pub fn validate_state_argument_specialization_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if candidate.rule()
        != OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.state-argument-specialization.v1",
        )
        || candidate.required_analyses()
            != AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::StronglyConnectedComponents,
            ])
        || candidate.invalidated_analyses()
            != AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::PostDominators,
                AnalysisKind::LoopForest,
                AnalysisKind::StronglyConnectedComponents,
                AnalysisKind::UseDefinition,
                AnalysisKind::ExecutableEdges,
                AnalysisKind::ScalarConstants,
                AnalysisKind::ValueRanges,
                AnalysisKind::EffectSummaries,
                AnalysisKind::ValueLiveness,
            ])
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    replay::validate(input, candidate)
}
