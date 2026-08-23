//! Producer-local fixed two-equality affine endpoint selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::super::super::affine_custody::DefinitionIndex;

use super::super::super::super::integer_evidence::cited_facts;

mod aliases;
mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for (outer_citation, outer_equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(outer_left, outer_right) = outer_equality else {
            continue;
        };
        for (old, middle_alias) in [(outer_left, outer_right), (outer_right, outer_left)] {
            let Some(endpoint) = aliases::outer(goal_left, goal_right, old, middle_alias) else {
                continue;
            };
            for (inner_citation, inner_equality) in cited_facts(assumptions, semantic_axioms) {
                if std::ptr::eq(outer_equality, inner_equality) {
                    continue;
                }
                let Proposition::Equal(inner_left, inner_right) = inner_equality else {
                    continue;
                };
                let Some(target_alias) = aliases::inner(old, middle_alias, inner_left, inner_right)
                else {
                    continue;
                };
                if let Some(proof) = completion::prove(
                    context,
                    goal,
                    goal_left,
                    goal_right,
                    middle_alias,
                    target_alias,
                    endpoint,
                    assumptions,
                    semantic_axioms,
                    definitions,
                    inner_citation,
                    inner_equality,
                    outer_citation,
                    outer_equality,
                ) {
                    return Some(proof);
                }
            }
        }
    }
    None
}
