//! Side-local evidence selection for exact integer-cast bounds.

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::cast_custody;

mod affine;
mod alias;
mod direct;
mod literal;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if !matches!(goal, Proposition::LessOrEqual(_, _)) {
        return None;
    }
    if let Some(proof) = direct::prove(context, goal, assumptions, semantic_axioms) {
        return Some(proof);
    }
    if let Some(proof) = prove_from_truth(context, goal, assumptions, semantic_axioms) {
        return Some(proof);
    }
    literal::prove(context, goal, assumptions, semantic_axioms)
        .or_else(|| alias::prove(context, goal, assumptions, semantic_axioms))
        .or_else(|| affine::prove(context, goal, assumptions, semantic_axioms))
}

fn prove_from_truth(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(left, right) = goal else {
        return None;
    };
    for target in [left, right]
        .into_iter()
        .filter(|term| matches!(term, ScalarTerm::Value { .. }))
    {
        let Some((root, _)) = cast_custody::source_root(target, semantic_axioms) else {
            continue;
        };
        let truth = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        };
        if let Some(proof) =
            cast_custody::prove_from_root(context, goal, assumptions, semantic_axioms, &root, truth)
        {
            return Some(proof);
        }
    }
    None
}
