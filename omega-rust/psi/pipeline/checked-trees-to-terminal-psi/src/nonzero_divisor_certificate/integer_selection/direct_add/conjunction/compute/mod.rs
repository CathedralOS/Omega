//! Ordered top-level conjunction assembly.

use proof_admission::ProofNode;
use semantic_vocabulary::{IntegerType, Proposition, PropositionContext, ScalarTerm};

use crate::nonzero_divisor_certificate::affine_custody::DefinitionIndex;
use crate::nonzero_divisor_certificate::integer_selection::dispatch::relax_math_bound;

use super::model::SearchState;

mod combine;
mod endpoint;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    target: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    state: &mut SearchState,
) -> Option<ProofNode> {
    let cutoff = semantic_axioms.len();
    let left = endpoint::derive(
        context,
        integer_type,
        left,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
        cutoff,
        0,
        state,
    )?;
    let right = endpoint::derive(
        context,
        integer_type,
        right,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
        cutoff,
        0,
        state,
    )?;
    if state.exhausted() {
        return None;
    }
    let mapped = combine::expression_bound(
        context,
        integer_type,
        target,
        lower,
        left,
        right,
        semantic_axioms,
    )?;
    (&mapped.conclusion == goal)
        .then_some(mapped.clone())
        .or_else(|| relax_math_bound(goal, mapped))
}
