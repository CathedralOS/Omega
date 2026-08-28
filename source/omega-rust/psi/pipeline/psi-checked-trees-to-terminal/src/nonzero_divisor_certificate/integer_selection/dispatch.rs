//! Canonical integer proposition-kind proof dispatch.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::{PrimitiveJudgment, ProofNode, ProofRule};

use super::super::affine_custody::DefinitionIndex;
use super::bound;

pub(super) fn prove_atomic(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<Option<ProofNode>> {
    match goal {
        Proposition::Truth => Some(Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        })),
        Proposition::LessOrEqual(_, _) => Some(bound::prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            definitions,
        )),
        _ => None,
    }
}
