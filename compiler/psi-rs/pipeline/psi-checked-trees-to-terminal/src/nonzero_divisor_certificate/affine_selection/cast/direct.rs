//! Direct cast-root custody for one following affine word.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};
use psi_proof_kernel::ProofNode;

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
        let (source, _) = cast_custody::source_root(cast_root, semantic_axioms)?;
        let cast_word = cast_custody::definition_axioms(&source, cast_root, semantic_axioms)?;
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
