use crate::context::*;
use crate::semantic_calls::find_state;

use super::accesses::{self, borrow_access_place};
use super::tracker::StateLoanTracker;
mod types;

use types::{is_mutable_reference_type, is_reference_type};

pub(super) fn statement_borrow_loan(
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
