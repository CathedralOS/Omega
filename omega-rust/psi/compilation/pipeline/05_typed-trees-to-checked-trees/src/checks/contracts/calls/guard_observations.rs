//! Compare selected guards with requirements using the same observation
//! identities as ordinary contract facts. Scalar copies, sealed predicates and
//! reference windows all retain their complete argument-effect interval.

use super::super::direct::GuardDenotations;
use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use typed_trees::expression::ExpressionHandle;

pub(super) fn conjunct_matches(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    guard: ExpressionHandle,
    required: ExpressionHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some((_, _, site, _)) = super::call_coordinates(program, state, call) else {
        return false;
    };
    let denotations = GuardDenotations {
        program,
        facts,
        caller_state_symbol: state.state_symbol,
        target_state_symbol: call.target_symbol,
        statement_index: call.statement_index,
        call_frames,
        arguments: validation::call_site_argument_expressions(program, &site),
        call,
        copies: std::cell::OnceCell::new(),
    };
    super::super::direct::guard_value_proves_with(
        program,
        guard,
        true,
        required,
        true,
        &|left, right| denotations.equal(left, right),
    )
}
