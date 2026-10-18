//! Capture and transport the loans contained in ordinary values.
//!
//! A call's returned leaf identifies exact storage for execution, while its
//! lifetime retains the supplied input region. Capture that prefix before
//! appending the callee's leaf projection. Keeping both judgments on one
//! captured path preserves selector and incoming-reference identity; widening
//! to the caller's root would incorrectly freeze unrelated sibling fields.
//! Carried values transport this obligation with the descriptor, even after
//! the original holder retires. Ordinary borrows have no call-input prefix.

use super::super::accesses::{self, borrow_access_place};
use super::super::tracker::{BorrowOwnerSegment, StateLoanTracker};
use super::StatementBorrowLoan;
use super::aggregate::{BorrowedInitializer, BorrowedInitializerKind, borrowed_initializers};
use super::call_views::*;
use super::owner_paths::{owner_path_matches, place_path_matches_owner_prefix};
use super::types::is_reference_type;
use crate::borrow::view_link::{ViewReturnFieldSource, is_mutably_borrow_carrying_data};
use checked_trees::expression::ExpressionHandle;
use symbols::SymbolHandle;
use validation::borrow_contracts::ViewReturnInputSource;

/// The loan created by constructing a borrow-carrying `data` value in a `let`:
/// the value borrows the source its reference field is initialized from.
/// Struct/case literals and fixed arrays are followed recursively so wrapping
/// one or several views in another checked aggregate cannot erase their loans.
/// A nested direct helper call or moved/projected aggregate is then expanded
/// under the enclosing field/index prefix with its original loan polarity.
pub(super) fn borrow_carrying_data_loans(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
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
            borrowed_initializer_loans(
                program,
                state.symbol,
                statement_index,
                machine_symbol,
                local_data,
                initializer,
                loan_trackers,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(in crate::borrow) fn borrowed_initializer_loans(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    initializer: BorrowedInitializer,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    match initializer.kind {
        BorrowedInitializerKind::Reference { access } => {
            if let Some(mut loans) = super::returned_carriers::result_loans(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                local_data,
                initializer.expression,
                &initializer.owner_path,
                loan_trackers,
            ) {
                super::returned_carriers::attenuate(&mut loans, &super::borrow_access_kind(access));
                return loans;
            }
            if let Some(mut loans) = stored_reference_loans(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                local_data,
                initializer.expression,
                &initializer.owner_path,
                loan_trackers,
            ) {
                super::returned_carriers::attenuate(&mut loans, &super::borrow_access_kind(access));
                return loans;
            }
            // Aggregate leaves obey the same source-selection law as call
            // arguments. A leaf may itself be a view-producing helper call;
            // routing it through the call-aware resolver keeps the selected
            // input loan instead of treating the call expression as no place.
            let Some(place) = argument_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                initializer.expression,
            ) else {
                return Vec::new();
            };
            let rebased = rebase_borrow_places_through_local_loans(program, place, loan_trackers);
            rebased
                .iter()
                .map(|source| StatementBorrowLoan {
                    incoming_origin: None,
                    formation_expression: initializer.expression,
                    owner_symbol: local_data.symbol,
                    storage_owner: facts::PlaceRoot::Symbol(local_data.symbol),
                    owner_name: local_data.name.clone(),
                    owner_path: initializer.owner_path.clone(),
                    place: source.place.clone(),
                    retained_operand: source.retained_operand,
                    source_owner_symbol: source.source_owner_symbol,
                    supplying_loans: Vec::new(),
                    lineage: retained_reference_lineage(
                        program,
                        initializer.expression,
                        source,
                        &rebased,
                    ),
                    kind: super::borrow_access_kind(access),
                    call_result: false,
                })
                .collect()
        }
        BorrowedInitializerKind::Aggregate { type_reference } => aggregate_expression_borrow_loans(
            program,
            state_symbol,
            statement_index,
            machine_symbol,
            local_data,
            type_reference,
            initializer.expression,
            &initializer.owner_path,
            loan_trackers,
        ),
    }
}

/// Transporting a stored descriptor keeps its exact historical suppliers.
/// A borrow, recast or returned view instead establishes its own relation;
/// equal backing alone cannot identify descriptor transport.
#[allow(clippy::too_many_arguments)]
pub(super) fn stored_reference_loans(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    expression: ExpressionHandle,
    owner_path: &[BorrowOwnerSegment],
    loan_trackers: &[StateLoanTracker],
) -> Option<Vec<StatementBorrowLoan>> {
    if validation::reference_formation_source(program, expression).is_some() {
        return None;
    }
    let reference = validation::expression_type_reference_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )?;
    validation::reference_shell(program, reference)?;
    let source = borrow_access_place(
        program,
        state_symbol,
        statement_index,
        expression,
        machine_symbol,
    )?;
    // Only the selected descriptor leaf supplies this value. An enclosing
    // reference's loan does not describe a descriptor stored in its referent.
    // Resolve containing references using the same current-storage traversal
    // as formation, but retain the final descriptor occurrence before its
    // pointee is normalized. Pointer geometry cannot select this supplier.
    let sources = rebase_borrow_places_through_local_loans(program, source, loan_trackers);
    let mut loans = Vec::new();
    for source in sources {
        let mut selected = loan_trackers.iter().filter(|loan| {
            loan.handle == source.parent_loan
                && loan.storage_owner == source.source_place.root
                && loan.owner_path.len() == source.source_place.segments.len()
                && owner_path_matches(program, &loan.owner_path, &source.source_place.segments)
        });
        let first = selected.next()?;
        loans.extend(transferred_aggregate_loans(
            program,
            local_data,
            expression,
            &source.source_place,
            owner_path,
            std::iter::once(first).chain(selected),
        ));
    }
    (!loans.is_empty()).then_some(loans)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn aggregate_expression_borrow_loans(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    type_reference: typed_trees::types::TypeReferenceHandle,
    expression: ExpressionHandle,
    owner_path_prefix: &[BorrowOwnerSegment],
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    match program.expression_table.expression(expression) {
        checked_trees::expression::ExpressionNode::Call(call) => {
            if let Some(field_loans) = super::returned_carriers::result_loans(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                local_data,
                expression,
                owner_path_prefix,
                loan_trackers,
            ) {
                return field_loans;
            }

            let Some(place) = helper_call_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                call,
            ) else {
                return Vec::new();
            };
            let rebased = rebase_borrow_places_through_local_loans(program, place, loan_trackers);
            rebased
                .iter()
                .map(|source| StatementBorrowLoan {
                    incoming_origin: None,
                    formation_expression: expression,
                    owner_symbol: local_data.symbol,
                    storage_owner: facts::PlaceRoot::Symbol(local_data.symbol),
                    owner_name: local_data.name.clone(),
                    owner_path: owner_path_prefix.to_vec(),
                    place: source.place.clone(),
                    retained_operand: source.retained_operand,
                    source_owner_symbol: source.source_owner_symbol,
                    supplying_loans: Vec::new(),
                    lineage: retained_reference_lineage(program, expression, source, &rebased),
                    kind: if is_mutably_borrow_carrying_data(program, type_reference) {
                        checked_trees::BorrowAccessKind::Mutable
                    } else {
                        checked_trees::BorrowAccessKind::Read
                    },
                    call_result: false,
                })
                .collect()
        }
        checked_trees::expression::ExpressionNode::Name(_)
        | checked_trees::expression::ExpressionNode::Member(_)
        | checked_trees::expression::ExpressionNode::Indexed(_) => {
            if let Some(loans) = super::returned_carriers::result_loans(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                local_data,
                expression,
                owner_path_prefix,
                loan_trackers,
            ) {
                return loans;
            }
            let Some(source) = borrow_access_place(
                program,
                state_symbol,
                statement_index,
                expression,
                machine_symbol,
            ) else {
                return Vec::new();
            };
            transferred_aggregate_loans(
                program,
                local_data,
                expression,
                &source,
                owner_path_prefix,
                loan_trackers.iter(),
            )
        }
        checked_trees::expression::ExpressionNode::Cast(cast) if !cast.form.is_recast() => {
            // A same-carrier value cast preserves denotation and can erase only
            // non-owning qualification. Re-expand its operand under the same
            // owner prefix so the cast cannot erase carried ownership loans.
            let nested = borrowed_initializers(
                program,
                cast.target_type,
                cast.value,
                &[],
                owner_path_prefix,
            );
            nested
                .into_iter()
                .flat_map(|initializer| {
                    borrowed_initializer_loans(
                        program,
                        state_symbol,
                        statement_index,
                        machine_symbol,
                        local_data,
                        initializer,
                        loan_trackers,
                    )
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Expression calls and named normal completions transport the same declared
/// result-to-input relation. The caller supplies its applied contract; this
/// operation composes actual captured leaves without inventing another call.
pub(super) fn result_source_loans(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
    fields: Vec<ViewReturnFieldSource>,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    owner_path_prefix: &[BorrowOwnerSegment],
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    let mut carried_arguments = Vec::new();

    let sources = super::returned_carriers::completed_sources(program, target, fields);
    sources
        .into_iter()
        .flat_map(|(field, projection)| {
            let mut owner_path = owner_path_prefix.to_vec();
            owner_path.extend(field.owner_path.iter().copied());
            let Some(argument) = arguments.get(field.parameter_index).copied() else {
                return Vec::new();
            };
            let source_type = CallViewReturnContract::carrier_reference(
                program,
                field.source_type,
                substitutions,
            );
            if !is_reference_type(program, source_type) {
                return super::returned_carriers::argument_loans(
                    program,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                    local_data,
                    argument,
                    &field,
                    &owner_path,
                    loan_trackers,
                    substitutions,
                    &mut carried_arguments,
                );
            }
            if field.source == ViewReturnInputSource::Referent
                && let Some(mut loans) = super::returned_carriers::relative_result_loans(
                    program,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                    local_data,
                    argument,
                    loan_trackers,
                )
            {
                super::returned_carriers::attenuate(&mut loans, &field.kind);
                for loan in &mut loans {
                    loan.owner_path = owner_path.clone();
                    loan.retained_operand
                        .get_or_insert(checked_trees::BorrowRetainedOperand {
                            formation_expression: Default::default(),
                            depth: loan.place.segments.len(),
                        });
                    loan.place.segments.extend_from_slice(&projection);
                }
                return loans;
            }
            let Some(mut place) = argument_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                argument,
            ) else {
                return Vec::new();
            };
            // A stored descriptor supplies its backing loan, not a new loan of
            // its container home. Select it before rebasing through captured
            // storage; a referent result instead projects after that rebase.
            if field.source == ViewReturnInputSource::Stored {
                if projection.is_empty() {
                    place
                        .segments
                        .extend(super::returned_carriers::place_segments(&field.source_path));
                } else {
                    place.segments.extend_from_slice(&projection);
                }
            }
            let rebased = rebase_borrow_places_through_local_loans(program, place, loan_trackers);
            rebased
                .iter()
                .map(|source| {
                    let mut place = source.place.clone();
                    if field.source == ViewReturnInputSource::Referent {
                        place.segments.extend_from_slice(&projection);
                    }
                    StatementBorrowLoan {
                        incoming_origin: None,
                        formation_expression: argument,
                        owner_symbol: local_data.symbol,
                        storage_owner: facts::PlaceRoot::Symbol(local_data.symbol),
                        owner_name: local_data.name.clone(),
                        owner_path: owner_path.clone(),
                        place,
                        retained_operand: Some(source.retained_operand.unwrap_or(
                            checked_trees::BorrowRetainedOperand {
                                formation_expression: Default::default(),
                                depth: source.place.segments.len(),
                            },
                        )),
                        source_owner_symbol: source.source_owner_symbol,
                        // A stored leaf or an unprojected referent result
                        // transports this exact actual descriptor. Preserve its
                        // supplier even when computed captured selectors cannot
                        // prove spatial containment later. A projected referent
                        // creates a child instead; backing coincidence cannot
                        // turn that child into a descriptor transfer.
                        supplying_loans: if source.parent_loan.is_valid()
                            && (field.source == ViewReturnInputSource::Stored
                                || projection.is_empty())
                        {
                            vec![source.parent_loan]
                        } else {
                            Vec::new()
                        },
                        lineage: retained_reference_lineage(program, argument, source, &rebased),
                        kind: field.kind.clone(),
                        call_result: false,
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn transferred_aggregate_loans<'loan>(
    program: &dyn ::validation::CheckingInput,
    local_data: &checked_trees::statement::TableLocalData,
    expression: ExpressionHandle,
    source: &accesses::BorrowAccessPlace,
    owner_path_prefix: &[BorrowOwnerSegment],
    loan_trackers: impl DoubleEndedIterator<Item = &'loan StateLoanTracker>,
) -> Vec<StatementBorrowLoan> {
    loan_trackers
        .rev()
        .filter_map(|loan| {
            if loan.storage_owner != source.root {
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

            let mut prefixed_owner_path = owner_path_prefix.to_vec();
            prefixed_owner_path.extend(owner_path);
            Some(StatementBorrowLoan {
                incoming_origin: None,
                formation_expression: expression,
                owner_symbol: local_data.symbol,
                storage_owner: facts::PlaceRoot::Symbol(local_data.symbol),
                owner_name: local_data.name.clone(),
                owner_path: prefixed_owner_path,
                place: loan.place.clone(),
                retained_operand: loan.retained_operand,
                source_owner_symbol: loan.owner_symbol,
                // Ingress custody lives in place.incoming_reference, not in
                // a local loan occurrence. Its zero handle adds no DAG edge.
                supplying_loans: std::iter::once(loan.handle)
                    .filter(|handle| handle.is_valid())
                    .collect(),
                lineage: loan.lineage.derived(),
                kind: loan.kind.clone(),
                call_result: false,
            })
        })
        .collect()
}
