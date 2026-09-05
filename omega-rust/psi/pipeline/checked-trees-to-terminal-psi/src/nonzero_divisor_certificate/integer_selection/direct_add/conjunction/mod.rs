//! Recursive direct-add conjunction search under one shared work budget.

use proof_admission::ProofNode;
use semantic_vocabulary::{IntegerType, Proposition, PropositionContext, ScalarTerm};

use crate::nonzero_divisor_certificate::affine_custody::DefinitionIndex;

mod compute;
mod definitions;
mod model;

use model::{SearchBudget, SearchState};

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
) -> Option<ProofNode> {
    let outcome = prove_with_budget(
        context,
        goal,
        integer_type,
        left,
        right,
        target,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
        SearchBudget::default(),
    );
    let _usage = outcome.usage;
    (!outcome.exhausted).then_some(outcome.proof).flatten()
}

#[allow(clippy::too_many_arguments)]
fn prove_with_budget(
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
    budget: SearchBudget,
) -> model::SearchOutcome {
    let mut state = SearchState::new(budget);
    let proof = compute::prove(
        context,
        goal,
        integer_type,
        left,
        right,
        target,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
        &mut state,
    );
    state.finish(proof)
}

#[cfg(test)]
mod tests;
