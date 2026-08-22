//! Canonical integer proposition-kind proof dispatch.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule};

use super::{bound, logical};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    mut prove_part: impl FnMut(&Proposition) -> Option<ProofNode>,
) -> Option<ProofNode> {
    match goal {
        Proposition::Truth => Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        }),
        Proposition::LessOrEqual(_, _) => bound::prove(context, goal, assumptions, semantic_axioms),
        Proposition::Conjunction(conjuncts) => {
            logical::prove_conjunction(goal, conjuncts, &mut prove_part)
        }
        Proposition::Disjunction(disjuncts) => {
            logical::prove_disjunction(goal, disjuncts, &mut prove_part)
        }
        _ => None,
    }
}
