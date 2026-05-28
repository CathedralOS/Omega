use crate::context::*;
mod accesses;
mod calls;
mod last_uses;
mod roots;

use crate::lookup::machine_state_count;
use crate::semantic_calls::find_state;
use accesses::borrow_access_place;
use calls::collect_statement_borrow_calls;
use last_uses::update_state_loan_last_uses;
use roots::{append_state_writable_roots, estimated_borrow_root_capacity, mutable_parameter_count};

#[derive(Clone)]
struct StateLoanTracker {
    handle: Handle<omega_checked_trees::BorrowLoanFact>,
    owner_symbol: SymbolHandle,
    owner_name: Identifier,
    place: accesses::BorrowAccessPlace,
}

pub(crate) fn build_borrow_facts(program: &omega_typed_trees::TypedTrees) -> BorrowFacts {
    let mut writable_roots =
        omega_core::arena::Arena::with_capacity(estimated_borrow_root_capacity(program));
    let mut access_segments =
        omega_core::arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut argument_accesses =
        omega_core::arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut calls =
        omega_core::arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut loans =
        omega_core::arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut states = omega_core::arena::Arena::with_capacity(machine_state_count(program));
    let mut state_loan_trackers = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            state_loan_trackers.clear();
            let mut writable_roots_span = omega_core::arena::HandleSpan::empty();
            append_state_writable_roots(
                program,
                machine,
                state,
                &mut writable_roots,
                &mut writable_roots_span,
            );

            let mut calls_span = omega_core::arena::HandleSpan::empty();
            let mut loans_span = omega_core::arena::HandleSpan::empty();
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                if let Some((owner_symbol, owner_name, place, source_owner_symbol, kind)) =
                    statement_borrow_loan(
                        program,
                        state,
                        statement_index,
                        machine.symbol,
                        statement,
                        &state_loan_trackers,
                    )
                {
                    let loan_segments = access_segments.insert_many(place.segments.clone());
                    let handle = loans.append_to_span(
                        &mut loans_span,
                        omega_checked_trees::BorrowLoanFact {
                            statement_index,
                            last_use_statement_index: statement_index,
                            owner_symbol,
                            source_owner_symbol,
                            root_symbol: place.root_symbol,
                            segments: loan_segments,
                            kind,
                        },
                    );
                    state_loan_trackers.push(StateLoanTracker {
                        handle,
                        owner_symbol,
                        owner_name,
                        place,
                    });
                }
                let mut call_ordinal = 0usize;
                collect_statement_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    statement,
                    &mut call_ordinal,
                    &mut access_segments,
                    &mut argument_accesses,
                    &mut calls,
                    &mut calls_span,
                );
            }

            update_state_loan_last_uses(
                program,
                state.statement_nodes,
                calls.span_or_empty(calls_span),
                &argument_accesses,
                &state_loan_trackers,
                &mut loans,
            );

            states.append(StateBorrowFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                writable_roots: writable_roots_span,
                mutable_parameter_count: mutable_parameter_count(program, state),
                calls: calls_span,
                loans: loans_span,
            });
        }
    }

    BorrowFacts {
        writable_roots,
        access_segments,
        argument_accesses,
        calls,
        loans,
        states,
    }
}

fn statement_borrow_loan(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    loan_trackers: &[StateLoanTracker],
) -> Option<(
    SymbolHandle,
    Identifier,
    accesses::BorrowAccessPlace,
    SymbolHandle,
    omega_checked_trees::BorrowAccessKind,
)> {
    match statement {
        StatementNode::LocalData(local_data) => {
            if !is_reference_type(program, local_data.type_reference) {
                return None;
            }

            let local_is_mutable_reference =
                is_mutable_reference_type(program, local_data.type_reference);
            let place = match program
                .expression_table
                .expression(local_data.initial_value)
            {
                omega_checked_trees::expression::ExpressionNode::Mutable(inner_expression)
                    if local_is_mutable_reference =>
                {
                    borrow_access_place(
                        program,
                        state.symbol,
                        statement_index,
                        *inner_expression,
                        machine_symbol,
                    )
                }
                omega_checked_trees::expression::ExpressionNode::Call(call) => {
                    helper_call_borrow_loan_place(
                        program,
                        state.symbol,
                        statement_index,
                        machine_symbol,
                        call,
                    )
                }
                omega_checked_trees::expression::ExpressionNode::Indexed(_) => borrow_access_place(
                    program,
                    state.symbol,
                    statement_index,
                    local_data.initial_value,
                    machine_symbol,
                ),
                _ => None,
            }?;

            let (place, source_owner_symbol) =
                rebase_borrow_place_through_local_loan(place, loan_trackers);

            Some((
                local_data.symbol,
                local_data.name.clone(),
                place,
                source_owner_symbol,
                if local_is_mutable_reference {
                    omega_checked_trees::BorrowAccessKind::Mutable
                } else {
                    omega_checked_trees::BorrowAccessKind::Read
                },
            ))
        }
        StatementNode::Assignment(_)
        | StatementNode::Call(_)
        | StatementNode::Expression(_)
        | StatementNode::Transition(_) => None,
    }
}

fn helper_call_borrow_loan_place(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    call: &omega_checked_trees::expression::TableCallExpression,
) -> Option<accesses::BorrowAccessPlace> {
    if !call.receiver.is_valid() {
        return None;
    }

    if matches!(
        call.target.as_str(),
        "as_slice" | "as_mut_slice" | "as_view"
    ) {
        return borrow_access_place(
            program,
            state_symbol,
            statement_index,
            call.receiver,
            machine_symbol,
        );
    }

    let Some(target_state) = find_state(program, call.target_symbol) else {
        return None;
    };
    let receiver_is_self = program
        .state_parameters(target_state)
        .iter()
        .any(|parameter| parameter.is_self);

    if !receiver_is_self || !is_reference_type(program, target_state.return_type) {
        return None;
    }

    borrow_access_place(
        program,
        state_symbol,
        statement_index,
        call.receiver,
        machine_symbol,
    )
}

fn rebase_borrow_place_through_local_loan(
    place: accesses::BorrowAccessPlace,
    loan_trackers: &[StateLoanTracker],
) -> (accesses::BorrowAccessPlace, SymbolHandle) {
    let Some(source_loan) = loan_trackers
        .iter()
        .rev()
        .find(|loan| loan.owner_symbol == place.root_symbol)
    else {
        return (place, SymbolHandle::invalid());
    };

    let mut rebased_segments = Vec::with_capacity(
        source_loan
            .place
            .segments
            .len()
            .saturating_add(place.segments.len()),
    );
    rebased_segments.extend(source_loan.place.segments.iter().copied());
    rebased_segments.extend(place.segments.iter().copied());

    (
        accesses::BorrowAccessPlace {
            root_symbol: source_loan.place.root_symbol,
            segments: rebased_segments,
        },
        source_loan.owner_symbol,
    )
}

fn is_reference_type(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { .. } => true,
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_reference_type(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::Generic { .. }
        | omega_typed_trees::types::TypeReferenceNode::Named { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => false,
    }
}

fn is_mutable_reference_type(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { is_mutable, .. } => *is_mutable,
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_mutable_reference_type(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::Generic { .. }
        | omega_typed_trees::types::TypeReferenceNode::Named { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => false,
    }
}
