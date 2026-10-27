//! Complete a transition target's extraction lifetimes from actual observations.
//! The early borrow producer cannot yet know the reaching stored operand graph.
//! After flow settles, re-run its same ordered collector with authenticated
//! observation/source correspondences. Only identical access rosters can gain
//! the existing copied-payload end before access zero; paths, kinds and loans
//! are unchanged. Nested operand invocations retain their own ordinary ends.

use checked_trees::CheckFacts;

pub(crate) fn retain_completed_dispatch_extractions(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
) {
    let calls = facts
        .borrow
        .states
        .iter()
        .flat_map(|(_, state)| {
            facts
                .borrow
                .calls
                .span_or_empty(state.calls)
                .iter()
                .cloned()
                .map(move |call| (state.machine_symbol, state.state_symbol, call))
        })
        .collect::<Vec<_>>();
    for (machine, state, call) in calls {
        let Some(machine) = program.machine_by_symbol(machine) else {
            continue;
        };
        let Some(state) = program.state_in_machine(machine, state) else {
            continue;
        };
        let Some((site, extraction)) = completed_invocation(
            program,
            crate::flow::CapturedSubjectFacts::from(&*facts),
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        ) else {
            continue;
        };
        let arguments = validation::call_site_argument_expressions(program, &site);
        let mut segments = arena::Arena::new();
        let mut accesses = arena::Arena::new();
        let expected = super::collect_completed_transition_target_argument_accesses(
            program,
            &mut segments,
            &mut accesses,
            arguments,
            state.symbol,
            call.statement_index,
            machine.symbol,
            call.target_symbol,
            extraction,
        );
        let Some(expected) = accesses.span(expected) else {
            continue;
        };
        let Some(actual) = facts.borrow.argument_accesses.span(call.accesses) else {
            continue;
        };
        if actual.len() != expected.len()
            || !actual.iter().zip(expected).all(|(actual, expected)| {
                actual.expression == expected.expression
                    && actual.root == expected.root
                    && actual.kind == expected.kind
                    && facts.borrow.access_segments.span(actual.segments)
                        == segments.span(expected.segments)
            })
        {
            continue;
        }
        for (actual, expected) in facts
            .borrow
            .argument_accesses
            .span_mut_or_empty(call.accesses)
            .iter_mut()
            .zip(expected)
        {
            if actual.end_before_access.is_some() && expected.end_before_access == Some(0) {
                actual.end_before_access = Some(0);
                actual.released_at = expected.released_at;
            }
        }
    }
}

/// Only the exact whole arm invocation inherits the dispatch's completed
/// payload copies. A nested call, even to the same declaration, does not.
/// Both publication and independent access replay use this same authority.
pub(super) fn completed_invocation<'program>(
    program: &'program dyn validation::CheckingInput,
    facts: crate::flow::CapturedSubjectFacts<'_>,
    machine: &'program typed_trees::machine::Machine,
    state: &'program typed_trees::state::State,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<(
    validation::CallSite<'program>,
    validation::DispatchExtraction,
)> {
    let site = validation::find_call_site_in_state(
        program,
        machine,
        state,
        statement_index,
        call_ordinal,
    )?;
    let target =
        validation::transition_call_target(program, machine, state, statement_index, call_ordinal)?;
    if !site.completes_transition(program, target) {
        return None;
    }
    let extraction = crate::flow::completed_dispatch_extraction(
        program,
        facts,
        machine,
        state,
        statement_index,
    )?;
    Some((site, extraction))
}
