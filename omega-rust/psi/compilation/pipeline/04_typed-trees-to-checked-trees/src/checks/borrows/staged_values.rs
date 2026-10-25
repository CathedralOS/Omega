//! Loans retained by completed values while an invocation stages its operands.
//!
//! A copied record stops observing its former home, but its reference leaves
//! still exclude overlapping exclusive accesses. Statement loan inventories do
//! not describe every intermediate generation installed by a nested call. Use
//! the actual captured descriptors, their immutable storage cuts and the shared
//! occurrence schedule instead. These are exclusion demands, never new access
//! authority. Lifetime roots cannot supply pointer geometry; an unavailable
//! descriptor stays potentially interfering, including a known-static opaque one.

use super::overlap::StatedOrderingPremise;
use crate::checks::ownership_occurrences::{InvocationIdentity, StatementOccurrence};
use checked_trees::{CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

mod captures;
mod conflicts;

pub(super) fn check<'program>(
    program: &'program dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: usize,
    occurrences: &[StatementOccurrence],
    premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    bounds: &mut Option<validation::ImmutableBoundLookup<'program>>,
) {
    let Some(machine) = program.machine_by_symbol(state.machine_symbol) else {
        return;
    };
    let Some(typed_state) = program.state_by_symbol(state.state_symbol) else {
        return;
    };
    let mut leaves = Vec::new();
    for call in facts
        .flow
        .control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|call| {
            call.statement_index == statement
                && call.authored_dispatch_role
                    != checked_trees::FlowCallDispatchRole::SavedSubjectRead
        })
    {
        let identity = match validation::find_call_site_in_state(
            program,
            machine,
            typed_state,
            statement,
            call.call_ordinal,
        ) {
            Some(validation::CallSite::Expression { expression, .. }) => {
                Some(InvocationIdentity::Expression(expression))
            }
            Some(validation::CallSite::Statement(_)) => Some(InvocationIdentity::Statement),
            Some(validation::CallSite::TransitionNamed { .. }) => {
                validation::transition_call_target(
                    program,
                    machine,
                    typed_state,
                    statement,
                    call.call_ordinal,
                )
                .map(InvocationIdentity::Transition)
            }
            None => None,
        };
        let mut completions = occurrences
            .iter()
            .enumerate()
            .filter(|(_, occurrence)| occurrence.invocation == identity && identity.is_some());
        let completion = completions.next().map(|(position, _)| position);
        let Some(until) = completion.filter(|_| completions.next().is_none()) else {
            if call.operands.handles().any(|operand| {
                let capture = facts.flow.control.operands.get(operand);
                crate::borrow::accesses::is_owned_copy_type(program, capture.type_reference)
                    && validation::borrow_contracts::is_borrow_carrying_data(
                        program,
                        capture.type_reference,
                    )
            }) {
                diagnostics.push(refusal(
                    program,
                    state,
                    statement,
                    call.authored_expression,
                    "a copied argument has no unique staged invocation boundary",
                ));
            }
            continue;
        };
        for operand in call.operands.handles() {
            captures::append(
                program,
                facts,
                state,
                operand,
                until,
                false,
                occurrences,
                &mut leaves,
            );
        }
    }
    conflicts::check(
        program,
        facts,
        state,
        statement,
        occurrences,
        &leaves,
        premises,
        diagnostics,
        bounds,
    );
}

/// Produced descriptors can have no authored expression or source span. Keep their refusal
/// locatable without pretending that zero identifies a source occurrence.
fn refusal(
    program: &dyn validation::CheckingInput,
    state: &FlowStateFact,
    statement: usize,
    expression: typed_trees::expression::ExpressionHandle,
    message: &str,
) -> Diagnostic {
    if expression.is_valid() {
        let span = program.expression_table.source_span(expression);
        if span.span.end > span.span.start {
            return Diagnostic::error(message).with_source_span(span);
        }
    }
    Diagnostic::error(format!(
        "{message} in machine `{}`, state `{}`, statement {statement}",
        crate::labels::symbol_name(program, state.machine_symbol),
        crate::labels::symbol_name(program, state.state_symbol)
    ))
}
