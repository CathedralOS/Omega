//! Direct cast-root custody for one following affine word.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::super::affine_custody::DefinitionIndex;
use super::super::super::{affine_custody, cast_custody};
use super::endpoint;

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
        let (source, _) = cast_custody::source_root(cast_root, semantic_axioms)?;
        let cast_word = cast_custody::definition_axioms(&source, cast_root, semantic_axioms)?;
        let last_cast = *cast_word.last()?;
        assumptions
            .iter()
            .enumerate()
            .find_map(|(assumption, root_bound)| {
                let cast_goal = endpoint::remap(root_bound, &source, cast_root, cast_type)?;
                let cast_bound = cast_custody::prove_from_root(
                    context,
                    &cast_goal,
                    assumptions,
                    semantic_axioms,
                    &source,
                    ProofNode {
                        conclusion: root_bound.clone(),
                        rule: ProofRule::Assumption { index: assumption },
                    },
                )?;
                affine_custody::prove_from_root_after(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    definitions,
                    cast_root,
                    last_cast,
                    cast_bound,
                )
            })
    })
}
