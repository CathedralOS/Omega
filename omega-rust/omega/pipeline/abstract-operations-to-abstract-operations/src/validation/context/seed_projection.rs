//! Initial seed reconstruction and accepted proof/frontier fact projection.

use super::context_projection::ContextProjection;
use super::*;

pub(super) fn validate_seed_projection(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
    projected_context: &ContextProjection,
    require_initial_revision: bool,
) -> Result<(), OptimizationUnitValidationError> {
    let context = input.context();
    let reconstructed = context
        .reconstructed_obligations()
        .obligations()
        .iter()
        .map(|row| (row.obligation.id, row))
        .collect::<BTreeMap<_, _>>();
    let accepted = context
        .accepted_facts()
        .iter()
        .map(|fact| (fact.obligation, fact))
        .collect::<BTreeMap<_, _>>();
    if reconstructed.len() != accepted.len() {
        let obligation = reconstructed
            .keys()
            .find(|id| !accepted.contains_key(id))
            .or_else(|| accepted.keys().find(|id| !reconstructed.contains_key(id)))
            .copied()
            .expect("different finite obligation maps have a differing key");
        return Err(OptimizationUnitValidationError::AcceptedObligationMismatch(
            obligation,
        ));
    }
    for (obligation, row) in &reconstructed {
        if accepted
            .get(obligation)
            .is_none_or(|fact| fact.proposition != row.obligation.proposition)
        {
            return Err(OptimizationUnitValidationError::AcceptedObligationMismatch(
                *obligation,
            ));
        }
    }

    let mut seed =
        optimization_unit::reconstruct_psi_optimization_unit_seed(input.plan(), unit.fuel_schedule)
            .map_err(|_| {
                OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch
            })?;
    immutable_custody::attach_verified_structural_context(&mut seed, context.module())?;
    if !immutable_custody::same_immutable_signature_custody(&seed, unit) {
        return Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch);
    }
    let mut projected_facts = Vec::new();
    for function in &seed.functions {
        for reference in &function.facts {
            let OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = reference
            else {
                continue;
            };
            let row = reconstructed.get(obligation).filter(|row| {
                row.owner
                    == terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                        machine: function.machine,
                        operation: *support,
                    }
            });
            let fact = accepted.get(obligation);
            let (Some(row), Some(fact)) = (row, fact) else {
                return Err(
                    OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch,
                );
            };
            if row.obligation.proposition != fact.proposition {
                return Err(
                    OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch,
                );
            }
            let proposition = terminal_codec::canonical_proposition_order_key(&fact.proposition)
                .map_err(OptimizationUnitValidationError::ContextIdentity)?;
            projected_facts.push(optimization_unit::AcceptedObligationFact::new(
                seed.psi,
                projected_context.proof_fingerprint,
                function.machine,
                *support,
                *obligation,
                proposition,
            ));
        }
    }
    let projected = optimization_unit::attach_accepted_obligation_facts(seed, projected_facts)
        .map_err(|_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
    let projected = optimization_unit::attach_proof_questions(
        projected,
        projected_context.proof_questions.clone(),
    )
    .map_err(|_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
    let projected = optimization_unit::attach_ownership_frontier_facts(
        projected,
        projected_context.ownership_frontiers.clone(),
    )
    .map_err(|_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
    if (require_initial_revision && projected.identity != unit.identity)
        || projected.accepted_obligation_facts != unit.accepted_obligation_facts
        || projected.proof_questions != unit.proof_questions
    {
        return Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch);
    }
    Ok(())
}
