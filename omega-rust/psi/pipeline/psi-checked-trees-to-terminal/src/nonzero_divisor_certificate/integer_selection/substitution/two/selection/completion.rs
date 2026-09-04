//! Producer-local completion of one eligible two-equality endpoint candidate.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_admission::{ProofNode, ProofRule};

use super::super::super::super::super::integer_evidence::Citation;
use super::super::super::super::super::{affine_custody::DefinitionIndex, affine_selection};

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    goal_left: &ScalarTerm,
    goal_right: &ScalarTerm,
    middle_alias: &ScalarTerm,
    target_alias: &ScalarTerm,
    endpoint: usize,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    allow_cast: bool,
    inner_citation: Citation,
    inner_equality: &Proposition,
    outer_citation: Citation,
    outer_equality: &Proposition,
) -> Option<ProofNode> {
    let relation = if endpoint == 0 {
        Proposition::LessOrEqual(target_alias.clone(), goal_right.clone())
    } else {
        Proposition::LessOrEqual(goal_left.clone(), target_alias.clone())
    };
    let affine = if allow_cast {
        affine_selection::prove_with_definitions(
            context,
            &relation,
            assumptions,
            semantic_axioms,
            definitions,
        )
    } else {
        affine_selection::prove_without_cast(
            context,
            &relation,
            assumptions,
            semantic_axioms,
            definitions,
        )
    }?;
    let middle_relation = if endpoint == 0 {
        Proposition::LessOrEqual(middle_alias.clone(), goal_right.clone())
    } else {
        Proposition::LessOrEqual(goal_left.clone(), middle_alias.clone())
    };
    let inner = ProofNode {
        conclusion: middle_relation,
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(affine),
            equality: Box::new(inner_citation.proof(inner_equality)),
            endpoint,
        },
    };
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(inner),
            equality: Box::new(outer_citation.proof(outer_equality)),
            endpoint,
        },
    })
}
