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
    if matches!(
        call.target.as_str(),
        "as_slice" | "as_mut_slice" | "as_view"
    ) {
        if !call.receiver.is_valid() {
            return None;
        }
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
    if !is_reference_type(program, target_state.return_type) {
        return None;
    }

    let parameters = program.state_parameters(target_state);
    let receiver_is_self = parameters.iter().any(|parameter| parameter.is_self);

    if receiver_is_self {
        // Rust's elision rule 3: a `&self`/`&mut self` method's returned view
        // borrows self (the call receiver), even when other ref params exist.
        if !call.receiver.is_valid() {
            return None;
        }
        return borrow_access_place(
            program,
            state_symbol,
            statement_index,
            call.receiver,
            machine_symbol,
        );
    }

    // Elision rule 1 (lifetimes stage 1): a machine returning a view with
    // EXACTLY ONE candidate ref input links the returned view's loan to that
    // input's argument place. Zero candidates or multiple candidates produce
    // no loan here; the multiple-candidate case is rejected at the machine's
    // declaration (see `checks::borrows::elision`), and the zero-candidate
    // case keeps the historical (conservative-free) behavior.
    let arguments = program.expression_table.expression_handles(call.arguments);
    let mut linked_argument: Option<ExpressionHandle> = None;
    let mut argument_index = 0usize;
    for parameter in parameters {
        if parameter.is_self {
            continue;
        }
        let argument = arguments.get(argument_index).copied();
        argument_index = argument_index.saturating_add(1);
        if !is_reference_type(program, parameter.type_reference) {
            continue;
        }
        if linked_argument.is_some() {
            return None;
        }
        linked_argument = argument;
    }

    argument_borrow_loan_place(
        program,
        state_symbol,
        statement_index,
        machine_symbol,
        linked_argument?,
    )
}

/// The loan place for a call argument that the called machine's returned view
/// borrows (elision rule 1). Handles arguments that are themselves
/// view-producing calls (`bag.cells.as_mut_slice()`, or a nested one-ref-input
/// machine call) by recursing into the call's own loan place.
fn argument_borrow_loan_place(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    argument: ExpressionHandle,
) -> Option<accesses::BorrowAccessPlace> {
    match program.expression_table.expression(argument) {
        omega_checked_trees::expression::ExpressionNode::Call(inner_call) => {
            helper_call_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                inner_call,
            )
        }
        omega_checked_trees::expression::ExpressionNode::Mutable(inner)
            if matches!(
                program.expression_table.expression(*inner),
                omega_checked_trees::expression::ExpressionNode::Call(_)
            ) =>
        {
            argument_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                *inner,
            )
        }
        _ => borrow_access_place(
            program,
            state_symbol,
            statement_index,
            argument,
            machine_symbol,
        ),
    }
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
