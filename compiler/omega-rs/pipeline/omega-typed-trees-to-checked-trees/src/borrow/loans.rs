use crate::borrow::view_link::{
    ViewReturnSource, is_borrow_carrying_data, is_mutably_borrow_carrying_data,
    resolve_view_return_source,
};
use crate::context::*;
use crate::semantic_calls::find_state;

use super::accesses::{self, borrow_access_place};
use super::tracker::{BorrowOwnerSegment, StateLoanTracker};
use aggregate::borrowed_initializers;
use owner_paths::{
    owner_path_from_place_segments, owner_path_matches, place_path_matches_owner_prefix,
};

mod aggregate;
mod owner_paths;
mod types;

use types::{is_mutable_reference_type, is_reference_type};

pub(super) struct StatementBorrowLoan {
    pub(super) owner_symbol: SymbolHandle,
    pub(super) owner_name: Identifier,
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) place: accesses::BorrowAccessPlace,
    pub(super) source_owner_symbol: SymbolHandle,
    pub(super) kind: omega_checked_trees::BorrowAccessKind,
}

/// The initializer expressions that supply the references structurally carried
/// by `type_reference`. Persistent-storage checking uses this same traversal so
/// its static-source exemption cannot disagree with local aggregate loan
/// attribution about which nested fields actually borrow.
pub(crate) fn borrow_initializer_expressions(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
    expression: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    borrowed_initializers(program, type_reference, expression, &[], &[])
        .into_iter()
        .map(|initializer| initializer.expression)
        .collect()
}

pub(super) fn statement_borrow_loans(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    match statement {
        StatementNode::Assignment(assignment) => assignment_borrow_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            assignment,
            loan_trackers,
        ),
        StatementNode::LocalData(local_data) => {
            // Borrow-carrying data local (`let msg: Message = Message { body: input }`):
            // the value borrows whatever its reference field is initialized from,
            // so its loan follows that source's place (decision 15 stage 2).
            if !is_reference_type(program, local_data.type_reference)
                && is_borrow_carrying_data(program, local_data.type_reference)
            {
                return borrow_carrying_data_loans(
                    program,
                    state,
                    statement_index,
                    machine_symbol,
                    local_data,
                    loan_trackers,
                );
            }
            if !is_reference_type(program, local_data.type_reference) {
                return Vec::new();
            }

            reference_local_borrow_loans(
                program,
                state,
                statement_index,
                machine_symbol,
                local_data,
                loan_trackers,
            )
        }
        StatementNode::AssemblyFact(_)
        | StatementNode::Call(_)
        | StatementNode::Expression(_)
        | StatementNode::Transition(_) => Vec::new(),
    }
}

/// Replacing an existing local or one of its fields must establish the same
/// loans as an equivalent `let` initializer. Persistent machine storage is
/// deliberately excluded here: loans that survive state transitions require
/// the separate outlives/persistent-storage contract rather than a state-local
/// tracker.
fn assignment_borrow_loans(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    assignment: &omega_checked_trees::statement::TableAssignment,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    let Some(target) = borrow_access_place(
        program,
        state.symbol,
        statement_index,
        assignment.target,
        machine_symbol,
    ) else {
        return Vec::new();
    };
    let Some(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.symbol == target.root_symbol).then_some(local)
        })
    else {
        return Vec::new();
    };
    let Some(target_type) = crate::flow::expression_type_reference_in_state(
        program,
        state.symbol,
        statement_index,
        assignment.target,
    ) else {
        return Vec::new();
    };
    if !is_reference_type(program, target_type) && !is_borrow_carrying_data(program, target_type) {
        return Vec::new();
    }

    let target_owner_path = owner_path_from_place_segments(program, &target.segments);
    let synthetic_local = omega_checked_trees::statement::TableLocalData {
        symbol: local.symbol,
        name: local.name.clone(),
        type_reference: target_type,
        initial_value: assignment.value,
        is_mutable: true,
    };

    let mut loans = if is_reference_type(program, target_type) {
        reference_local_borrow_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            &synthetic_local,
            loan_trackers,
        )
    } else {
        borrow_carrying_data_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            &synthetic_local,
            loan_trackers,
        )
    };
    for loan in &mut loans {
        let mut path = target_owner_path.clone();
        path.append(&mut loan.owner_path);
        loan.owner_path = path;
    }
    loans
}

fn reference_local_borrow_loans(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &omega_checked_trees::statement::TableLocalData,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    let local_is_mutable_reference = is_mutable_reference_type(program, local_data.type_reference);
    let Some(place) = (match program
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
        omega_checked_trees::expression::ExpressionNode::Indexed(_)
        | omega_checked_trees::expression::ExpressionNode::Member(_)
        | omega_checked_trees::expression::ExpressionNode::Name(_) => borrow_access_place(
            program,
            state.symbol,
            statement_index,
            local_data.initial_value,
            machine_symbol,
        ),
        _ => None,
    }) else {
        return Vec::new();
    };

    rebase_borrow_places_through_local_loans(program, place, loan_trackers)
        .into_iter()
        .map(|(place, source_owner_symbol)| StatementBorrowLoan {
            owner_symbol: local_data.symbol,
            owner_name: local_data.name.clone(),
            owner_path: Vec::new(),
            place,
            source_owner_symbol,
            kind: if local_is_mutable_reference {
                omega_checked_trees::BorrowAccessKind::Mutable
            } else {
                omega_checked_trees::BorrowAccessKind::Read
            },
        })
        .collect()
}

/// The loan created by constructing a borrow-carrying `data` value in a `let`:
/// the value borrows the source its reference field is initialized from.
/// Struct/case literals and fixed arrays are followed recursively so wrapping
/// one or several views in another checked aggregate cannot erase their loans.
fn borrow_carrying_data_loans(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &omega_checked_trees::statement::TableLocalData,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    // A call-produced aggregate has no field initializer expressions to walk.
    // Its explicit result lifetime still identifies one source argument, so
    // attach that source to the aggregate as a whole. Field and nested uses
    // overlap this root path and therefore keep the source loan live.
    if let omega_checked_trees::expression::ExpressionNode::Call(call) = program
        .expression_table
        .expression(local_data.initial_value)
    {
        let field_loans = helper_call_aggregate_borrow_loans(
            program,
            state.symbol,
            statement_index,
            machine_symbol,
            local_data,
            call,
            loan_trackers,
        );
        if !field_loans.is_empty() {
            return field_loans;
        }

        let Some(place) = helper_call_borrow_loan_place(
            program,
            state.symbol,
            statement_index,
            machine_symbol,
            call,
        ) else {
            return Vec::new();
        };
        return rebase_borrow_places_through_local_loans(program, place, loan_trackers)
            .into_iter()
            .map(|(place, source_owner_symbol)| StatementBorrowLoan {
                owner_symbol: local_data.symbol,
                owner_name: local_data.name.clone(),
                owner_path: Vec::new(),
                place,
                source_owner_symbol,
                kind: if is_mutably_borrow_carrying_data(program, local_data.type_reference) {
                    omega_checked_trees::BorrowAccessKind::Mutable
                } else {
                    omega_checked_trees::BorrowAccessKind::Read
                },
            })
            .collect();
    }

    // Moving or copying a borrow-carrying local/projection must transfer every
    // loan contained by that value. The source local's tracked projection is
    // rebased to the new owner's root; no initializer literal exists to
    // rediscover the original borrowed expressions.
    if matches!(
        program
            .expression_table
            .expression(local_data.initial_value),
        omega_checked_trees::expression::ExpressionNode::Name(_)
            | omega_checked_trees::expression::ExpressionNode::Member(_)
            | omega_checked_trees::expression::ExpressionNode::Indexed(_)
    ) {
        if let Some(source) = borrow_access_place(
            program,
            state.symbol,
            statement_index,
            local_data.initial_value,
            machine_symbol,
        ) {
            let transferred =
                transferred_aggregate_loans(program, local_data, &source, loan_trackers);
            if !transferred.is_empty() {
                return transferred;
            }
        }
    }

    let initializers = borrowed_initializers(
        program,
        local_data.type_reference,
        local_data.initial_value,
        &[],
        &[],
    );

    initializers
        .into_iter()
        .flat_map(|initializer| {
            let Some(place) = borrow_access_place(
                program,
                state.symbol,
                statement_index,
                initializer.expression,
                machine_symbol,
            ) else {
                return Vec::new();
            };
            rebase_borrow_places_through_local_loans(program, place, loan_trackers)
                .into_iter()
                .map(|(place, source_owner_symbol)| StatementBorrowLoan {
                    owner_symbol: local_data.symbol,
                    owner_name: local_data.name.clone(),
                    owner_path: initializer.owner_path.clone(),
                    place,
                    source_owner_symbol,
                    kind: if initializer.is_mutable {
                        omega_checked_trees::BorrowAccessKind::Mutable
                    } else {
                        omega_checked_trees::BorrowAccessKind::Read
                    },
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn helper_call_aggregate_borrow_loans(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &omega_checked_trees::statement::TableLocalData,
    call: &omega_checked_trees::expression::TableCallExpression,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    let Some(target_state) = find_state(program, call.target_symbol) else {
        return Vec::new();
    };
    let ViewReturnSource::Fields { fields } = resolve_view_return_source(program, target_state)
    else {
        return Vec::new();
    };
    let arguments = program.expression_table.expression_handles(call.arguments);

    fields
        .into_iter()
        .flat_map(|field| {
            let Some(argument) = arguments.get(field.non_self_index).copied() else {
                return Vec::new();
            };
            let Some(place) = argument_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                argument,
            ) else {
                return Vec::new();
            };
            rebase_borrow_places_through_local_loans(program, place, loan_trackers)
                .into_iter()
                .map(|(place, source_owner_symbol)| StatementBorrowLoan {
                    owner_symbol: local_data.symbol,
                    owner_name: local_data.name.clone(),
                    owner_path: field.owner_path.clone(),
                    place,
                    source_owner_symbol,
                    kind: field.kind.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn transferred_aggregate_loans(
    program: &omega_typed_trees::TypedTrees,
    local_data: &omega_checked_trees::statement::TableLocalData,
    source: &accesses::BorrowAccessPlace,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    loan_trackers
        .iter()
        .rev()
        .filter_map(|loan| {
            if loan.owner_symbol != source.root_symbol {
                return None;
            }

            let owner_path =
                if place_path_matches_owner_prefix(program, &source.segments, &loan.owner_path) {
                    loan.owner_path[source.segments.len()..].to_vec()
                } else if owner_path_matches(program, &loan.owner_path, &source.segments) {
                    // A whole-aggregate call loan has an empty owner path. Any
                    // projection selected from that aggregate retains the loan.
                    Vec::new()
                } else {
                    return None;
                };

            Some(StatementBorrowLoan {
                owner_symbol: local_data.symbol,
                owner_name: local_data.name.clone(),
                owner_path,
                place: loan.place.clone(),
                source_owner_symbol: loan.owner_symbol,
                kind: loan.kind.clone(),
            })
        })
        .collect()
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
        ViewReturnSource::NotApplicable
        | ViewReturnSource::Ambiguous(_)
        | ViewReturnSource::Fields { .. } => None,
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

fn rebase_borrow_places_through_local_loans(
    program: &omega_typed_trees::TypedTrees,
    place: accesses::BorrowAccessPlace,
    loan_trackers: &[StateLoanTracker],
) -> Vec<(accesses::BorrowAccessPlace, SymbolHandle)> {
    let source_loans: Vec<&StateLoanTracker> = loan_trackers
        .iter()
        .rev()
        .filter(|loan| {
            loan.owner_symbol == place.root_symbol
                && owner_path_matches(program, &loan.owner_path, &place.segments)
        })
        .collect();
    if source_loans.is_empty() {
        return vec![(place, SymbolHandle::invalid())];
    }

    source_loans
        .into_iter()
        .map(|source_loan| {
            let remainder = &place.segments[source_loan.owner_path.len()..];
            let mut rebased_segments = Vec::with_capacity(
                source_loan
                    .place
                    .segments
                    .len()
                    .saturating_add(remainder.len()),
            );
            rebased_segments.extend(source_loan.place.segments.iter().copied());
            rebased_segments.extend(remainder.iter().copied());

            (
                accesses::BorrowAccessPlace {
                    root_symbol: source_loan.place.root_symbol,
                    segments: rebased_segments,
                },
                source_loan.owner_symbol,
            )
        })
        .collect()
}
