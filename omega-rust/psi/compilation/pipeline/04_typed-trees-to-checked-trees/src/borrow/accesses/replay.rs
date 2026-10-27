//! Rejoin an expression invocation's ordered access roster to its source.
//!
//! Named transition targets complete their extraction lifetimes from flow's
//! observed operand graph. Expression invocations keep ordinary argument-order
//! ends. Access rows are observations, not formal positions;
//! callers select operands through the exact authored call site returned here.

use validation::{CallSite, call_site_argument_expressions, find_call_site};

use checked_trees::{BorrowFacts, FlowCallFact};
use symbols::SymbolHandle;

pub(crate) fn rejoin_expression_call_accesses<'program>(
    program: &'program dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    source: &FlowCallFact,
) -> Option<CallSite<'program>> {
    let mut states = borrow
        .states
        .iter()
        .map(|(_, row)| row)
        .filter(|row| row.machine_symbol == machine && row.state_symbol == state);
    let state_row = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let mut calls = borrow.calls.span(state_row.calls)?.iter().filter(|call| {
        call.statement_index == source.statement_index && call.call_ordinal == source.call_ordinal
    });
    let call = calls.next()?;
    if calls.next().is_some()
        || call.target_symbol != source.target_symbol
        || call.accesses != source.accesses
        || call.has_receiver != source.has_receiver
        || call.receiver_symbol != source.receiver_symbol
    {
        return None;
    }
    let site = find_call_site(
        program,
        machine,
        state,
        source.statement_index,
        source.call_ordinal,
    )?;
    let collector = match &site {
        CallSite::Expression { expression, call } => {
            if *expression != source.authored_expression
                || validation::call_target_entry_symbol(
                    program,
                    validation::call_target_symbol(program, call),
                ) != source.target_symbol
            {
                return None;
            }
            super::collect_call_argument_accesses
        }
        _ => return None,
    };
    let arguments = call_site_argument_expressions(program, &site);
    let mut expected_segments = arena::Arena::new();
    let mut expected_accesses = arena::Arena::new();
    let expected = collector(
        program,
        &mut expected_segments,
        &mut expected_accesses,
        arguments,
        state,
        source.statement_index,
        machine,
        source.target_symbol,
    );
    let expected = expected_accesses.span(expected)?;
    let actual = borrow.argument_accesses.span(call.accesses)?;
    if expected.len() != actual.len() {
        return None;
    }
    for (expected, actual) in expected.iter().zip(actual) {
        if expected.expression != actual.expression
            || expected.root != actual.root
            || expected.kind != actual.kind
            || expected.end_before_access != actual.end_before_access
            || expected.released_at != actual.released_at
            || expected_segments.span(expected.segments)?
                != borrow.access_segments.span(actual.segments)?
        {
            return None;
        }
    }
    Some(site)
}
