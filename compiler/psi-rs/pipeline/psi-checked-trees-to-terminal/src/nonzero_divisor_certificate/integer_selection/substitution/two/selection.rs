//! Producer-local fixed two-equality affine endpoint selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::super::super::integer_evidence::cited_facts;

mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for (outer_citation, outer_equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(outer_left, outer_right) = outer_equality else {
            continue;
        };
        for (old, middle_alias) in [(outer_left, outer_right), (outer_right, outer_left)] {
            let endpoint = if old == goal_left {
                0
            } else if old == goal_right {
                1
            } else {
                continue;
            };
            if !matches!(old, psi_core::ScalarTerm::Value { .. })
                || !matches!(middle_alias, psi_core::ScalarTerm::Value { .. })
                || old == middle_alias
                || old.scalar_type() != middle_alias.scalar_type()
            {
                continue;
            }
            for (inner_citation, inner_equality) in cited_facts(assumptions, semantic_axioms) {
                if std::ptr::eq(outer_equality, inner_equality) {
                    continue;
                }
                let Proposition::Equal(inner_left, inner_right) = inner_equality else {
                    continue;
                };
                let target_alias = if inner_left == middle_alias {
                    inner_right
                } else if inner_right == middle_alias {
                    inner_left
                } else {
                    continue;
                };
                if !matches!(target_alias, psi_core::ScalarTerm::Value { .. })
                    || target_alias == old
                    || target_alias == middle_alias
                    || target_alias.scalar_type() != old.scalar_type()
                {
                    continue;
                }
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
