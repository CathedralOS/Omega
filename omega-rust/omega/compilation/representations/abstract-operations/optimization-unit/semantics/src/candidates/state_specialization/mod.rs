//! Optimizer module role: executable entrance. State-argument specialization validation.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod replay;

pub(crate) use replay::cyclic_machines;

pub fn validate_state_argument_specialization_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::contract::validate(
        input,
        candidate,
        b"omega.psi-rule.state-argument-specialization.v1",
        AnalysisSet::new([
            AnalysisKind::ScalarConstants,
            AnalysisKind::StronglyConnectedComponents,
        ]),
        AnalysisInvalidationSet::new([
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
        ]),
        OptimizationSafetyClass::StructuralIdentity,
    )?;
    if !candidate.substitutions().is_empty() {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    replay::validate(input, candidate)
}
