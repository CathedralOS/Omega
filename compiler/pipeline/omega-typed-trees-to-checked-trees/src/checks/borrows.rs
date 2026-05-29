mod calls;
mod details;
mod overlap;
mod statements;

use omega_checked_trees::{CheckFacts, FlowStateFact};
use omega_core::diagnostics::Diagnostic;

use self::calls::check_call_borrows;
use self::statements::check_statement_borrows;

pub(crate) fn check_flow_call_borrows(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(borrow_state) = matching_borrow_state(facts, state_flow) else {
            continue;
        };

        for borrow_call in facts.borrow.calls.span_or_empty(borrow_state.calls) {
            check_call_borrows(program, facts, state_flow, borrow_call, &mut diagnostics);
        }

        check_statement_borrows(program, facts, state_flow, &mut diagnostics);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn matching_borrow_state<'a>(
    facts: &'a CheckFacts,
    state_flow: &FlowStateFact,
) -> Option<&'a omega_checked_trees::StateBorrowFact> {
    facts.borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == state_flow.machine_symbol
            && state.state_symbol == state_flow.state_symbol)
            .then_some(state)
    })
}
