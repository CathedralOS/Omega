mod calls;
mod details;
mod elision;
mod escape;
mod overlap;
mod persistent;
mod statements;

use psi_checked_trees::{CheckFacts, FlowStateFact};
use psi_diagnostics::Diagnostic;

use self::calls::check_call_borrows;
use self::elision::check_view_return_elision;
use self::escape::check_view_return_escape;
use self::persistent::check_persistent_borrow_assignments;
use self::statements::check_statement_borrows;

pub(crate) fn check_flow_call_borrows(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut compatibility_certificates = Vec::new();

    // Checked recording is deliberately idempotent: each run rebuilds this
    // proof ledger from the unchanged resource/control facts.
    facts
        .borrow
        .compatibility_certificates
        .reset_retain_capacity();

    check_view_return_elision(program, &mut diagnostics);
    check_view_return_escape(program, facts, &mut diagnostics);
    check_persistent_borrow_assignments(program, &mut diagnostics);

    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(borrow_state) = matching_borrow_state(facts, state_flow) else {
            continue;
        };

        for borrow_call in facts.borrow.calls.span_or_empty(borrow_state.calls) {
            check_call_borrows(program, facts, state_flow, borrow_call, &mut diagnostics);
        }

        check_statement_borrows(
            program,
            facts,
            state_flow,
            &mut diagnostics,
            &mut compatibility_certificates,
        );
    }

    facts
        .borrow
        .compatibility_certificates
        .insert_many(compatibility_certificates);

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn matching_borrow_state<'a>(
    facts: &'a CheckFacts,
    state_flow: &FlowStateFact,
) -> Option<&'a psi_checked_trees::StateBorrowFact> {
    facts.borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == state_flow.machine_symbol
            && state.state_symbol == state_flow.state_symbol)
            .then_some(state)
    })
}
