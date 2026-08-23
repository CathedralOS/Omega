//! Ordered direct cast-to-affine candidates for production.

use psi_core::{Proposition, PropositionContext, ScalarType};
use psi_proof_kernel::ProofNode;

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
        let last_cast = *cast_word.last()?;
        assumptions
            .iter()
            .enumerate()
            .find_map(|(assumption, root_bound)| {
                completion::prove(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    definitions,
                    &source,
                    cast_root,
                    cast_type,
                    last_cast,
                    assumption,
                    root_bound,
                )
            })
    })
}
