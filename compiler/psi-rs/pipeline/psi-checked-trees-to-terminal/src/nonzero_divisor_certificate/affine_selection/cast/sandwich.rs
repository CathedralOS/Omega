//! Direct-root affine/cast/affine certificate composition.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::super::affine_custody::DefinitionIndex;
use super::super::super::cast_custody;

mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Option<ProofNode> {
    semantic_axioms.iter().find_map(|axiom| {
        let Proposition::Equal(cast_root, ScalarTerm::IntegerExactCast { .. }) = axiom else {
            return None;
        };
        let ScalarType::Integer(cast_type) = cast_root.scalar_type() else {
            return None;
        };
        let (source, first_cast) = cast_custody::source_root(cast_root, semantic_axioms)?;
        let cast_word = cast_custody::definition_axioms(&source, cast_root, semantic_axioms)?;
        let last_cast = *cast_word.last()?;
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
