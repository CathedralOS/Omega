use crate::context::*;
use crate::borrow::view_link::{
    ViewReturnSource, is_borrow_carrying_data, resolve_view_return_source,
};
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
            // Borrow-carrying data local (`let msg: Message = Message { body: input }`):
            // the value borrows whatever its reference field is initialized from,
            // so its loan follows that source's place (decision 15 stage 2).
            if !is_reference_type(program, local_data.type_reference)
                && is_borrow_carrying_data(program, local_data.type_reference)
            {
                return borrow_carrying_data_loan(
                    program,
                    state,
                    statement_index,
                    machine_symbol,
                    local_data,
                    loan_trackers,
                );
            }
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

/// The loan created by constructing a borrow-carrying `data` value in a `let`:
/// the value borrows the source its reference field is initialized from. Only
/// struct-literal construction is tracked for now.
fn borrow_carrying_data_loan(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &omega_checked_trees::statement::TableLocalData,
    loan_trackers: &[StateLoanTracker],
) -> Option<(
    SymbolHandle,
    Identifier,
    accesses::BorrowAccessPlace,
    SymbolHandle,
    omega_checked_trees::BorrowAccessKind,
)> {
    let ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(local_data.initial_value)
    else {
        return None;
    };
    let (field_value, is_mutable) =
        borrowed_field_initializer(program, local_data.type_reference, literal)?;
    let place = borrow_access_place(
        program,
        state.symbol,
        statement_index,
        field_value,
        machine_symbol,
    )?;
    let (place, source_owner_symbol) = rebase_borrow_place_through_local_loan(place, loan_trackers);
    Some((
        local_data.symbol,
        local_data.name.clone(),
        place,
        source_owner_symbol,
        if is_mutable {
            omega_checked_trees::BorrowAccessKind::Mutable
        } else {
            omega_checked_trees::BorrowAccessKind::Read
        },
    ))
}

/// For a borrow-carrying `data` type's struct literal, the initializer
/// expression of its (first) reference-typed field plus whether that reference
/// is mutable. Common fields only for the first cut (case payloads deferred).
fn borrowed_field_initializer(
    program: &omega_typed_trees::TypedTrees,
    data_type_reference: omega_typed_trees::types::TypeReferenceHandle,
    literal: &omega_checked_trees::expression::TableStructLiteral,
) -> Option<(ExpressionHandle, bool)> {
    let symbol = program.type_reference_symbol(data_type_reference);
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)?;
    let fields = program.expression_table.struct_fields(literal.fields);
    for member in program.data_members(definition) {
        let omega_typed_trees::data::DataMember::Field(field) = member else {
            continue;
        };
        if !is_reference_type(program, field.type_reference) {
            continue;
        }
        let is_mutable = is_mutable_reference_type(program, field.type_reference);
        if let Some(literal_field) = fields
            .iter()
            .find(|literal_field| literal_field.name.as_str() == field.name.as_str())
        {
            return Some((literal_field.value, is_mutable));
        }
    }
    None
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

    let target_state = find_state(program, call.target_symbol)?;

    // The borrow source (self, a named input, or none) is resolved by the same
    // logic the declaration check uses (`borrow::view_link`), so the loan we
    // track here always matches what the elision check accepted. Elision rules
    // 1/3 and stage-2 explicit lifetimes all flow through there.
    match resolve_view_return_source(program, target_state) {
        ViewReturnSource::NotApplicable | ViewReturnSource::Ambiguous(_) => None,
        ViewReturnSource::SelfReceiver => {
            // Elision rule 3: a `&self`/`&mut self` method's returned view
            // borrows self (the call receiver).
            if !call.receiver.is_valid() {
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
        ViewReturnSource::Parameter { non_self_index } => {
            // The returned view borrows one named (or single) ref input; its
            // loan follows that argument's place. Arguments map 1:1 to non-self
            // parameters (a `&self` receiver routes through `SelfReceiver`).
            let arguments = program.expression_table.expression_handles(call.arguments);
            let argument = arguments.get(non_self_index).copied()?;
            argument_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                argument,
            )
        }
    }
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
