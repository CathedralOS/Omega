//! Fixed one-equality integer-bound substitution production.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};

use super::super::super::affine_custody::DefinitionIndex;

use super::super::super::integer_evidence::cited_facts;

mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    allow_cast: bool,
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };

    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(equality_left, equality_right) = fact else {
            continue;
        };
        for (old, replacement) in [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ] {
            // Completion substitutes only at a goal endpoint; skip the
            // citation's proof copy for orientations it would refuse.
            if old != goal_left && old != goal_right {
                continue;
            }
            if let Some(proof) = completion::prove(
                context,
                goal,
                goal_left,
                goal_right,
                old,
                replacement,
                assumptions,
                semantic_axioms,
                definitions,
                allow_cast,
                citation.proof(fact),
            ) {
                return Some(proof);
            }
        }
    }

    None
}
