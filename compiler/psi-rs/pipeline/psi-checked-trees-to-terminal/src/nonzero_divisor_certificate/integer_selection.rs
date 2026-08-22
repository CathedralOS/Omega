//! Canonical fixed-integer proposition and bound selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule};

mod bound;
mod exact;
mod logical;
mod order;
mod substitution;

pub(super) fn build(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if let Some(proof) = exact::prove(goal, assumptions, semantic_axioms) {
        return Some(proof);
    }
    match goal {
        Proposition::Truth => Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        }),
        Proposition::LessOrEqual(_, _) => bound::prove(context, goal, assumptions, semantic_axioms),
        Proposition::Conjunction(conjuncts) => {
            logical::prove_conjunction(goal, conjuncts, |part| {
                build(context, part, assumptions, semantic_axioms)
            })
        }
        Proposition::Disjunction(disjuncts) => {
            logical::prove_disjunction(goal, disjuncts, |part| {
                build(context, part, assumptions, semantic_axioms)
            })
        }
        _ => None,
    }
}
