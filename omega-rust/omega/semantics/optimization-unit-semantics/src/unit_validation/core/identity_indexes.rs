//! Canonical unit identity plus accepted proof/frontier fact indexes.

use super::*;

pub(super) fn validate_identity_and_fact_indexes(
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    let recomputed = recompute_psi_optimization_unit_identity(unit);
    if unit.identity != recomputed {
        return Err(OptimizationUnitValidationError::ContentIdentityMismatch {
            stored: unit.identity,
            recomputed,
        });
    }
    if unit.fuel_schedule != TerminalFuelSchedule::CURRENT.identity() {
        return Err(OptimizationUnitValidationError::WrongFuelSchedule);
    }
    if unit
        .accepted_obligation_facts
        .iter()
        .any(|fact| fact.psi != unit.psi || !fact.has_canonical_identity())
        || unit.accepted_obligation_facts.windows(2).any(|pair| {
            (pair[0].machine, pair[0].operation, pair[0].obligation)
                >= (pair[1].machine, pair[1].operation, pair[1].obligation)
        })
    {
        return Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch);
    }
    let mut proof_question_identities = BTreeSet::new();
    let mut proof_question_owners = BTreeSet::new();
    if unit.proof_questions.iter().any(|question| {
        question.terminal_psi != unit.psi
            || !question.has_canonical_identity()
            || !proof_question_identities.insert(question.identity)
            || !proof_question_owners.insert((question.owner, question.obligation))
    }) {
        return Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch);
    }
    if unit.ownership_frontier_facts.iter().any(|fact| {
        fact.psi != unit.psi
            || !fact.has_canonical_identity()
            || !canonical_ownership_frontier_snapshot(&fact.snapshot)
    }) || unit
        .ownership_frontier_facts
        .windows(2)
        .any(|pair| (pair[0].machine, pair[0].site) >= (pair[1].machine, pair[1].site))
    {
        return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
    }
    Ok(())
}
