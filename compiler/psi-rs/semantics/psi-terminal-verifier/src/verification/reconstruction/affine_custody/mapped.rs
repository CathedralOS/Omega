//! Independent exact affine mapping strictly before one source boundary.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{check_integer_affine_bound_conversion, check_integer_affine_witness};

use super::{DefinitionIndex, candidates, relaxation};

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn retained_mapped_to_target_before(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
    maximum_axiom: usize,
    root_bound: &Proposition,
) -> Option<Proposition> {
    candidates::find_target(
        context,
        semantic_axioms,
        definitions,
        root,
        target,
        |witness| {
            if !witness
                .definition_axioms
                .iter()
                .all(|&index| index < maximum_axiom)
                || !witness
                    .literal_axioms
                    .iter()
                    .flatten()
                    .all(|&index| index < maximum_axiom)
            {
                return None;
            }
            let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
            let mapped = relaxation::mapped_bound(&form, root_bound)?;
            check_integer_affine_bound_conversion(&form, root_bound, &mapped)
                .is_ok()
                .then_some(mapped)
        },
    )
}
