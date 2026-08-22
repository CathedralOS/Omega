//! Canonical fixed-integer proposition and bound selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule};

use super::integer_evidence::cited_facts;
use super::{affine_selection, cast_selection};

mod logical;
mod order;
mod substitution;

pub(super) fn build(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        if fact == goal {
            return Some(citation.proof(fact));
        }
    }
    match goal {
        Proposition::Truth => Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        }),
        Proposition::LessOrEqual(_, _) => {
            prove_integer_bound(context, goal, assumptions, semantic_axioms)
        }
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

fn prove_integer_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if let Some(proof) =
        order::prove_exact_or_closed_transitive_integer_bound(goal, assumptions, semantic_axioms)
    {
        return Some(proof);
    }

    if let Some(proof) =
        order::prove_two_fact_transitive_integer_bound(goal, assumptions, semantic_axioms)
    {
        return Some(proof);
    }

    if let Some(proof) = substitution::prove(context, goal, assumptions, semantic_axioms) {
        return Some(proof);
    }

    cast_selection::prove(context, goal, assumptions, semantic_axioms)
        .or_else(|| affine_selection::prove(context, goal, assumptions, semantic_axioms))
}
