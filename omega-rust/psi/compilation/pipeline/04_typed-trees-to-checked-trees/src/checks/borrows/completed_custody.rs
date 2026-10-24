//! Availability of owned parents at an implicit transformation's completion.
//!
//! Numeric conservation cannot retire borrowed storage. Use the same ordered
//! occurrences and spatial judgments as explicit moves, including loans made
//! by preceding operands and accesses held by an enclosing invocation. These
//! parents are already exact owned claim places: resolving the *result*
//! expression as their source would substitute an unrelated storage identity.
//! This query excludes candidates; it grants no output membership or custody.

use crate::checks::ownership_occurrences::{
    StatementStep, routes_compatible, statement_steps_at_evaluation,
};
use checked_trees::{BorrowAccessKind, CapturedPlace, CheckFacts, FlowOperandFact};
use validation::CanonicalPlace;

pub(crate) fn require_unborrowed_completion(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    operand: &FlowOperandFact,
    evaluation: checked_trees::FlowPermissionEvaluation,
    parents: &[CanonicalPlace],
) -> Result<(), &'static str> {
    let state_flow = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, flow)| {
            (flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol)
                .then_some(flow)
        })
        .ok_or("missing completion state flow")?;
    let borrow_state =
        super::matching_borrow_state(facts, state_flow).ok_or("missing completion borrow state")?;
    let statement = facts
        .flow
        .control
        .statements
        .span_or_empty(state_flow.statements)
        .iter()
        .find(|statement| statement.statement_index == operand.statement_index)
        .ok_or("missing completion statement flow")?;
    let node = program
        .statement_table
        .statement_at(state, operand.statement_index)
        .ok_or("missing completion statement")?;
    let moves = facts
        .flow
        .ownership
        .storage_transfers
        .span_or_empty(state_flow.storage_transfers);
    let occurrences = statement_steps_at_evaluation(
        program,
        machine,
        state,
        operand.statement_index,
        node,
        moves,
        &facts.operators,
        Some(evaluation),
    );
    let mut completions = occurrences.iter().enumerate().filter(|(_, occurrence)| {
        matches!(occurrence.step, StatementStep::Complete(expression)
            if expression == operand.expression)
    });
    let (position, completion) = completions
        .next()
        .ok_or("missing exact completion occurrence")?;
    if completions.next().is_some() {
        return Err("ambiguous completion occurrence");
    }
    let final_holds = super::reads::final_holds_in_statement(
        program,
        facts,
        state_flow,
        statement,
        node,
        &occurrences,
        false,
    );
    let mut active = facts
        .flow
        .borrow_loan_constraints(statement.entry_constraints)
        .filter(|loan| {
            !final_holds
                .iter()
                .any(|(ended, held_until)| ended == loan && *held_until <= position)
        })
        .collect::<Vec<_>>();
    for (handle, loan) in facts.borrow.loans.iter() {
        if facts.borrow.state_owns_loan(borrow_state, handle)
            && loan.statement_index == operand.statement_index
            && occurrences[..=position].iter().any(|occurrence| {
                matches!(occurrence.step, StatementStep::Complete(expression)
                    if expression == loan.formation_expression)
                    && routes_compatible(&occurrence.route, &completion.route)
            })
            && !active.contains(&handle)
        {
            active.push(handle);
        }
    }
    // No later statement or eventual return premise enters this query.
    // With no stated ordering premise an unresolved spatial overlap remains
    // interfering, as it does for an ordinary move without such a premise.
    let mut bounds = None;
    for parent in parents {
        for handle in &active {
            if !super::overlap::canonical_place_loan_compatibility(
                program,
                parent,
                facts.borrow.loans.get(*handle),
                &facts.borrow,
                &[],
                &mut bounds,
            )
            .non_interfering
            {
                return Err("input still has a live loan at completion");
            }
        }
    }

    // Returning reference leaves have no local owner and therefore live in
    // the returned-loan inventory, not `loans`. They still exclude consuming
    // their backing when that same completed result is being established.
    for loan in facts
        .borrow
        .returned_loans
        .span_or_empty(borrow_state.returned_loans)
    {
        let loan_evaluation = if loan.transition_target.is_valid() {
            checked_trees::FlowPermissionEvaluation::TransitionTarget(loan.transition_target)
        } else {
            checked_trees::FlowPermissionEvaluation::Statement
        };
        if loan.statement_index != operand.statement_index
            || loan_evaluation != evaluation
            || !occurrences[..=position].iter().any(|occurrence| {
                matches!(occurrence.step, StatementStep::Complete(expression)
                    if expression == loan.formation_expression)
                    && routes_compatible(&occurrence.route, &completion.route)
            })
        {
            continue;
        }
        let borrowed = CapturedPlace {
            incoming_reference: loan.incoming_reference,
            root: loan.root,
            segments: facts
                .borrow
                .access_segments
                .span_or_empty(loan.segments)
                .to_vec(),
        };
        for parent in parents {
            let owned = CapturedPlace {
                incoming_reference: Default::default(),
                root: parent.root,
                segments: parent.segments.clone(),
            };
            if !super::overlap::captured_place_compatibility(
                program,
                &borrowed,
                &loan.kind,
                &owned,
                &BorrowAccessKind::Mutable,
                &[],
                &mut bounds,
            )
            .non_interfering
            {
                return Err("completed result still borrows its input");
            }
        }
    }

    // Call operands also lend storage without creating a named local loan.
    // Their ordered access roster distinguishes a completed copied read from
    // a reference still held across a later operand's completion.
    for call in facts
        .borrow
        .calls
        .span_or_empty(borrow_state.calls)
        .iter()
        .filter(|call| call.statement_index == operand.statement_index)
    {
        let flow_call = facts
            .flow
            .control
            .calls
            .span_or_empty(state_flow.calls)
            .iter()
            .find(|candidate| {
                candidate.statement_index == call.statement_index
                    && candidate.call_ordinal == call.call_ordinal
            })
            .ok_or("missing staged invocation occurrence")?;
        if !occurrences[position + 1..].iter().any(|occurrence| {
            matches!(occurrence.step, StatementStep::Complete(expression)
                if expression == flow_call.authored_expression)
                && routes_compatible(&occurrence.route, &completion.route)
        }) {
            continue;
        }
        let accesses = facts.borrow.argument_accesses.span_or_empty(call.accesses);
        for access in accesses {
            let observed = occurrences[..position].iter().any(|occurrence| {
                matches!(occurrence.step, StatementStep::Observe { expression, .. }
                    if expression == access.expression)
                    && routes_compatible(&occurrence.route, &completion.route)
            });
            if !observed
                || super::calls::access_released_in_prefix(
                    access,
                    &occurrences[..=position],
                    &completion.route,
                )
            {
                continue;
            }
            if occurrences[..position].iter().any(|occurrence| {
                let StatementStep::Move {
                    event,
                    ordered: true,
                    ..
                } = occurrence.step
                else {
                    return false;
                };
                let previous = &moves[event];
                previous.expression == access.expression
                    && routes_compatible(&occurrence.route, &completion.route)
                    && crate::checks::multiplicity::transfers_owned_storage(
                        program,
                        state.symbol,
                        operand.statement_index,
                        previous,
                        facts
                            .flow
                            .ownership
                            .segments
                            .span_or_empty(previous.segments),
                    )
            }) {
                continue;
            }
            let backings = super::place_access::place_backings(
                program,
                facts,
                state_flow,
                operand.statement_index,
                active.iter().copied(),
                CapturedPlace {
                    incoming_reference: Default::default(),
                    root: access.root,
                    segments: facts.borrow.access_segments(access).to_vec(),
                },
            )
            .ok_or("staged input loan has no exact backing")?;
            for parent in parents {
                let owned = CapturedPlace {
                    incoming_reference: Default::default(),
                    root: parent.root,
                    segments: parent.segments.clone(),
                };
                if backings.iter().any(|backing| {
                    !super::overlap::captured_place_compatibility(
                        program,
                        &backing.place,
                        &access.kind,
                        &owned,
                        &BorrowAccessKind::Mutable,
                        &[],
                        &mut bounds,
                    )
                    .non_interfering
                }) {
                    return Err("input is still borrowed by an enclosing invocation");
                }
            }
        }
    }
    Ok(())
}
