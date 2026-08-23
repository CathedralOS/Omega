//! Producer-local completion of one oriented endpoint equality.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::super::relation;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    goal_left: &ScalarTerm,
    goal_right: &ScalarTerm,
    old: &ScalarTerm,
    replacement: &ScalarTerm,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    equality: ProofNode,
) -> Option<ProofNode> {
    let (endpoint, relation) = if old == goal_left {
        (
            0,
            Proposition::LessOrEqual(replacement.clone(), goal_right.clone()),
        )
    } else if old == goal_right {
        (
            1,
            Proposition::LessOrEqual(goal_left.clone(), replacement.clone()),
        )
    } else {
        return None;
    };
    let relation = relation::prove(
        context,
        &relation,
        replacement.integer_value().is_some(),
        assumptions,
        semantic_axioms,
        definitions,
    )?;
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(relation),
            equality: Box::new(equality),
            endpoint,
        },
    })
}
