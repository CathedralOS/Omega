//! Ordered affine/cast/affine candidates for production.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};
use psi_proof_admission::{ProofNode, ProofRule};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let cast_roots = definitions.cast_roots().cloned().collect::<Vec<_>>();
    cast_roots.iter().find_map(|cast_root| {
        let ScalarType::Integer(cast_type) = cast_root.scalar_type() else {
            return None;
        };
        let (source, cast_word) = definitions.cast_spine(cast_root)?;
        let first_cast = cast_word[0];
        let last_cast = *cast_word.last()?;
        if !super::super::super::super::affine_custody::has_target_after(
            context,
            goal,
            semantic_axioms,
            definitions,
            cast_root,
            last_cast,
        ) {
            return None;
        }
        assumptions
            .iter()
            .enumerate()
            .find_map(|(assumption, root_bound)| {
                let Proposition::LessOrEqual(left, right) = root_bound else {
                    return None;
                };
                [left, right].into_iter().find_map(|root| {
                    if !matches!(root, ScalarTerm::Value { .. }) || root == &source {
                        return None;
                    }
                    if !super::super::super::super::affine_custody::has_target_before(
                        context,
                        semantic_axioms,
                        definitions,
                        root,
                        &source,
                        first_cast,
                    ) {
                        return None;
                    }
                    completion::prove(
                        context,
                        goal,
                        assumptions,
                        semantic_axioms,
                        definitions,
                        root,
                        &source,
                        first_cast,
                        cast_root,
                        cast_type,
                        last_cast,
                        ProofNode {
                            conclusion: root_bound.clone(),
                            rule: ProofRule::Assumption { index: assumption },
                        },
                    )
                })
            })
    })
}
