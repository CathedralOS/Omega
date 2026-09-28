//! Producer-local relaxed completion of one checked affine witness.

use std::rc::Rc;

use proof_admission::{
    CheckedIntegerAffineForm, IntegerAffineWitness, ProofNode, check_certificate,
};
use semantic_vocabulary::{Proposition, PropositionContext};

use super::super::{DefinitionIndex, relaxation};

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    form: &Rc<CheckedIntegerAffineForm>,
    root_bound: &ProofNode,
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
    let relaxed = relaxation::prove(goal, definitions, form, root_bound, witness)?;
    check_certificate(context, goal, assumptions, semantic_axioms, &relaxed)
        .is_ok()
        .then_some(relaxed)
}
