use super::{VerifiedPsiOptimizationContext, VerifiedPsiOptimizationUnitBuildError};

pub(super) fn project_accepted_obligation_facts(
    seed: &optimization_unit::PsiOptimizationUnit,
    context: &VerifiedPsiOptimizationContext,
) -> Result<Vec<optimization_unit::AcceptedObligationFact>, VerifiedPsiOptimizationUnitBuildError> {
    let proof_fingerprint = *context.proof_bundle_fingerprint().as_bytes();
    let mut facts = Vec::new();
    for function in &seed.functions {
        for reference in &function.facts {
            let optimization_unit::OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = reference
            else {
                continue;
            };
            let reconstructed = context
                .reconstructed_obligations()
                .obligations()
                .iter()
                .find(|row| {
                    row.obligation.id == *obligation
                        && row.owner
                            == terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                                machine: function.machine,
                                operation: *support,
                            }
                })
                .ok_or(
                    VerifiedPsiOptimizationUnitBuildError::MissingReconstructedObligation {
                        machine: function.machine,
                        operation: *support,
                        obligation: *obligation,
                    },
                )?;
            let accepted = context
                .accepted_facts()
                .iter()
                .find(|fact| fact.obligation == *obligation)
                .filter(|fact| fact.proposition == reconstructed.obligation.proposition)
                .ok_or(
                    VerifiedPsiOptimizationUnitBuildError::MissingAcceptedObligation {
                        machine: function.machine,
                        operation: *support,
                        obligation: *obligation,
                    },
                )?;
            let proposition =
                terminal_codec::canonical_proposition_order_key(&accepted.proposition)?;
            facts.push(optimization_unit::AcceptedObligationFact::new(
                seed.psi,
                proof_fingerprint,
                function.machine,
                *support,
                *obligation,
                proposition,
            ));
        }
    }
    Ok(facts)
}
