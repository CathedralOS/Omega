//! Producer-local affine-to-cast completion for one oriented target.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};
use psi_proof_admission::ProofNode;

use super::super::super::{cast_custody, integer_selection};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    target: &ScalarTerm,
    literal: &ScalarTerm,
    target_is_right: bool,
) -> Option<ProofNode> {
    let (source, first_cast) = cast_custody::source_root(target, semantic_axioms)?;
    let ScalarType::Integer(source_type) = source.scalar_type() else {
        return None;
    };
    let literal = cast_custody::remap_integer_literal(literal, source_type)?;
    let source_goal = if target_is_right {
        Proposition::LessOrEqual(literal, source.clone())
    } else {
        Proposition::LessOrEqual(source.clone(), literal)
    };
    let root_bound = integer_selection::build(
        context,
        &source_goal,
        assumptions,
        &semantic_axioms[..first_cast],
    )?;
    cast_custody::prove_from_root(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &source,
        root_bound,
    )
}
