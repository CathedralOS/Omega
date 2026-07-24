use crate::borrow::view_link::{
    ViewReturnSource, is_borrow_carrying_data, is_mutably_borrow_carrying_data,
    resolve_view_return_source,
};
use crate::context::*;
use crate::semantic_calls::find_state;

use super::accesses::{self, borrow_access_place};
use super::tracker::{BorrowOwnerSegment, StateLoanTracker};
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

pub(super) fn statement_borrow_loans(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    match statement {
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

            let local_is_mutable_reference =
                is_mutable_reference_type(program, local_data.type_reference);
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
        StatementNode::AssemblyFact(_)
        | StatementNode::Assignment(_)
        | StatementNode::Call(_)
        | StatementNode::Expression(_)
        | StatementNode::Transition(_) => Vec::new(),
    }
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

#[derive(Clone)]
struct BorrowedInitializer {
    owner_path: Vec<BorrowOwnerSegment>,
    expression: ExpressionHandle,
    is_mutable: bool,
}

/// Resolve every structurally carried reference initializer, its projection
/// within the aggregate owner, and its polarity.
fn borrowed_initializers(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
    expression: ExpressionHandle,
    substitutions: &[(SymbolHandle, omega_typed_trees::types::TypeReferenceHandle)],
    owner_path: &[BorrowOwnerSegment],
) -> Vec<BorrowedInitializer> {
    use omega_typed_trees::types::TypeReferenceNode;

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { is_mutable, .. } => vec![BorrowedInitializer {
            owner_path: owner_path.to_vec(),
            expression,
            is_mutable: *is_mutable,
        }],
        TypeReferenceNode::Constrained { base_type, .. } => {
            borrowed_initializers(program, *base_type, expression, substitutions, owner_path)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            let ExpressionNode::ArrayLiteral(values) =
                program.expression_table.expression(expression)
            else {
                return Vec::new();
            };
            program
                .expression_table
                .expression_handles(*values)
                .iter()
                .enumerate()
                .flat_map(|(index, value)| {
                    let mut element_path = owner_path.to_vec();
                    element_path.push(BorrowOwnerSegment::FixedIndex(index));
                    borrowed_initializers(
                        program,
                        *element_type,
                        *value,
                        substitutions,
                        &element_path,
                    )
                })
                .collect()
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *base_symbol)
            else {
                return Vec::new();
            };
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let mut nested_substitutions = substitutions.to_vec();
            nested_substitutions.extend(
                program
                    .data_type_parameters(definition)
                    .iter()
                    .zip(arguments.iter())
                    .map(|(parameter, argument)| (parameter.symbol, *argument)),
            );
            borrowed_data_literal_initializers(
                program,
                definition,
                expression,
                &nested_substitutions,
                owner_path,
            )
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some((_, concrete)) = substitutions
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == symbol)
            {
                return borrowed_initializers(
                    program,
                    *concrete,
                    expression,
                    substitutions,
                    owner_path,
                );
            }
            let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol)
            else {
                return Vec::new();
            };
            borrowed_data_literal_initializers(
                program,
                definition,
                expression,
                substitutions,
                owner_path,
            )
        }
        TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => Vec::new(),
    }
}

fn borrowed_data_literal_initializers(
    program: &omega_typed_trees::TypedTrees,
    definition: &omega_typed_trees::data::DataDefinition,
    expression: ExpressionHandle,
    substitutions: &[(SymbolHandle, omega_typed_trees::types::TypeReferenceHandle)],
    owner_path: &[BorrowOwnerSegment],
) -> Vec<BorrowedInitializer> {
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        return Vec::new();
    };
    let literal_fields = program.expression_table.struct_fields(literal.fields);

    let fields: Vec<&omega_typed_trees::data::DataField> =
        if let Some(case_name) = &literal.case_name {
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    omega_typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == case_name.as_str() =>
                    {
                        Some(program.data_payload_fields(variant).iter().collect())
                    }
                    _ => None,
                })
                .unwrap_or_default()
        } else {
            program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    omega_typed_trees::data::DataMember::Field(field) => Some(field),
                    omega_typed_trees::data::DataMember::Variant(_) => None,
                })
                .collect()
        };

    fields
        .into_iter()
        .flat_map(|field| {
            let Some(literal_field) = literal_fields
                .iter()
                .find(|literal_field| literal_field.name.as_str() == field.name.as_str())
            else {
                return Vec::new();
            };
            let mut field_path = owner_path.to_vec();
            field_path.push(BorrowOwnerSegment::Field(field.symbol));
            borrowed_initializers(
                program,
                field.type_reference,
                literal_field.value,
                substitutions,
                &field_path,
            )
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

fn owner_path_matches(
    program: &omega_typed_trees::TypedTrees,
    owner_path: &[BorrowOwnerSegment],
    place_segments: &[omega_facts::PlaceSegment],
) -> bool {
    owner_path.len() <= place_segments.len()
        && owner_path
            .iter()
            .zip(place_segments)
            .all(|(owner, place)| match (owner, place) {
                (
                    BorrowOwnerSegment::Field(owner_symbol),
                    omega_facts::PlaceSegment::Field {
                        symbol: place_symbol,
                    },
                ) => !place_symbol.is_valid() || owner_symbol == place_symbol,
                (
                    BorrowOwnerSegment::FixedIndex(owner_index),
                    omega_facts::PlaceSegment::Index { expression },
                ) => program
                    .expression_table
                    .constant_integer_value(*expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_none_or(|place_index| *owner_index == place_index),
                _ => false,
            })
}
