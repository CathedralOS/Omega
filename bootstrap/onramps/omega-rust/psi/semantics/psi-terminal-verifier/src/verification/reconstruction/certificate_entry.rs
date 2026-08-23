//! Independent entry from canonical scalar goals to retained proof evidence.

use psi_core::{Proposition, PropositionContext};
use psi_terminal_semantics::CanonicalScalarGoal;

use super::{affine_custody::DefinitionIndex, integer_selection};

pub(super) fn retained(
    context: Option<&PropositionContext>,
    goal: &CanonicalScalarGoal,
    semantic_axioms: &[Proposition],
    requirements: &[Proposition],
) -> bool {
    let Ok(Some(proposition)) = goal.kernel_proposition() else {
        return false;
    };
    let mut definitions = DefinitionIndex::new(semantic_axioms);
    integer_selection::retained(
        context,
        &proposition,
        requirements,
        semantic_axioms,
        &mut definitions,
    )
}
