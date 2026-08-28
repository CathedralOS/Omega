use crate::borrow::view_link::{
    ViewReturnSource, is_borrow_carrying_data, is_mutably_borrow_carrying_data,
    resolve_signature_view_return_source, resolve_view_return_source,
};
use crate::context::*;
use crate::semantic_calls::find_state;

use super::accesses::{self, borrow_access_place};
use super::tracker::{BorrowOwnerSegment, StateLoanTracker};
use aggregate::{BorrowedInitializer, BorrowedInitializerKind, borrowed_initializers};
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
    pub(super) lineage: psi_checked_trees::BorrowLoanLineage,
    pub(super) kind: psi_checked_trees::BorrowAccessKind,
}

struct RebasedBorrowPlace {
    place: accesses::BorrowAccessPlace,
    source_owner_symbol: SymbolHandle,
    parent_loan: Handle<psi_checked_trees::BorrowLoanFact>,
    parent_lineage_is_retained: bool,
}

/// The initializer expressions that supply the references structurally carried
/// by `type_reference`. Persistent-storage checking uses this same traversal so
/// its static-source exemption cannot disagree with local aggregate loan
/// attribution about which nested fields actually borrow.
pub(crate) fn borrow_initializer_expressions(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    expression: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    borrowed_initializers(program, type_reference, expression, &[], &[])
        .into_iter()
        .map(|initializer| initializer.expression)
        .collect()
}

pub(super) fn statement_borrow_loans(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
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
                true,
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
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    assignment: &psi_checked_trees::statement::TableAssignment,
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
    let synthetic_local = psi_checked_trees::statement::TableLocalData {
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
            false,
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
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &psi_checked_trees::statement::TableLocalData,
    loan_trackers: &[StateLoanTracker],
    allow_direct_reborrow_lineage: bool,
) -> Vec<StatementBorrowLoan> {
    let local_is_mutable_reference = is_mutable_reference_type(program, local_data.type_reference);
    let is_explicit_reborrow = matches!(
        program
            .expression_table
            .expression(local_data.initial_value),
        psi_checked_trees::expression::ExpressionNode::Borrow(_)
    );
    let force_unretained = matches!(
        program
            .expression_table
            .expression(local_data.initial_value),
        psi_checked_trees::expression::ExpressionNode::Call(_)
            | psi_checked_trees::expression::ExpressionNode::Cast(_)
    );
    let Some(place) = (match program
        .expression_table
        .expression(local_data.initial_value)
    {
        psi_checked_trees::expression::ExpressionNode::Borrow(inner_expression)
            if local_is_mutable_reference =>
        {
            whole_place_recast_borrow_place(
                program,
                state.symbol,
                statement_index,
                inner_expression.target,
                machine_symbol,
            )
            .or_else(|| {
                borrow_access_place(
                    program,
                    state.symbol,
                    statement_index,
                    inner_expression.target,
                    machine_symbol,
                )
            })
        }
        psi_checked_trees::expression::ExpressionNode::Cast(cast) if cast.form.is_recast() => {
            whole_place_recast_borrow_place(
                program,
                state.symbol,
                statement_index,
                local_data.initial_value,
                machine_symbol,
            )
        }
        psi_checked_trees::expression::ExpressionNode::Call(call) => helper_call_borrow_loan_place(
            program,
            state.symbol,
            statement_index,
            machine_symbol,
            call,
        ),
        psi_checked_trees::expression::ExpressionNode::Indexed(_)
        | psi_checked_trees::expression::ExpressionNode::Member(_)
        | psi_checked_trees::expression::ExpressionNode::Name(_) => borrow_access_place(
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

    let rebased = rebase_borrow_places_through_local_loans(program, place, loan_trackers);
    rebased
        .iter()
        .map(|source| StatementBorrowLoan {
            owner_symbol: local_data.symbol,
            owner_name: local_data.name.clone(),
            owner_path: Vec::new(),
            place: source.place.clone(),
            source_owner_symbol: source.source_owner_symbol,
            lineage: retained_reference_lineage(
                source,
                &rebased,
                allow_direct_reborrow_lineage && is_explicit_reborrow,
                force_unretained,
            ),
            kind: if local_is_mutable_reference {
                psi_checked_trees::BorrowAccessKind::Mutable
            } else {
                psi_checked_trees::BorrowAccessKind::Read
            },
        })
        .collect()
}

fn whole_place_recast_borrow_place(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    machine_symbol: SymbolHandle,
) -> Option<accesses::BorrowAccessPlace> {
    let psi_checked_trees::expression::ExpressionNode::Cast(cast) =
        program.expression_table.expression(expression)
    else {
        return None;
    };
    if !cast.form.is_recast()
        || !matches!(
            program.expression_table.expression(cast.value),
            psi_checked_trees::expression::ExpressionNode::Name(_)
                | psi_checked_trees::expression::ExpressionNode::Member(_)
        )
    {
        return None;
    }

    // Whole name/member recasts retain exactly the source place's provenance
    // and lifetime. Indexed byte-region recasts are intentionally excluded:
    // their validated target footprint may cover more than one source element,
    // so an element-only loan would understate overlap.
    borrow_access_place(
        program,
        state_symbol,
        statement_index,
        cast.value,
        machine_symbol,
    )
}

/// The loan created by constructing a borrow-carrying `data` value in a `let`:
/// the value borrows the source its reference field is initialized from.
/// Struct/case literals and fixed arrays are followed recursively so wrapping
/// one or several views in another checked aggregate cannot erase their loans.
/// A nested direct helper call or moved/projected aggregate is then expanded
/// under the enclosing field/index prefix with its original loan polarity.
fn borrow_carrying_data_loans(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &psi_checked_trees::statement::TableLocalData,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    if matches!(
        program
            .expression_table
            .expression(local_data.initial_value),
        psi_checked_trees::expression::ExpressionNode::Call(_)
            | psi_checked_trees::expression::ExpressionNode::Name(_)
            | psi_checked_trees::expression::ExpressionNode::Member(_)
            | psi_checked_trees::expression::ExpressionNode::Indexed(_)
    ) {
        return aggregate_expression_borrow_loans(
            program,
            state.symbol,
            statement_index,
            machine_symbol,
            local_data,
            local_data.type_reference,
            local_data.initial_value,
            &[],
            loan_trackers,
        );
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
fn borrowed_initializer_loans(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &psi_checked_trees::statement::TableLocalData,
    initializer: BorrowedInitializer,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    match initializer.kind {
        BorrowedInitializerKind::Reference { is_mutable } => {
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
            rebase_borrow_places_through_local_loans(program, place, loan_trackers)
                .into_iter()
                .map(|source| StatementBorrowLoan {
                    owner_symbol: local_data.symbol,
                    owner_name: local_data.name.clone(),
                    owner_path: initializer.owner_path.clone(),
                    place: source.place,
                    source_owner_symbol: source.source_owner_symbol,
                    lineage: psi_checked_trees::BorrowLoanLineage::UnretainedDerived,
                    kind: if is_mutable {
                        psi_checked_trees::BorrowAccessKind::Mutable
                    } else {
                        psi_checked_trees::BorrowAccessKind::Read
                    },
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

#[allow(clippy::too_many_arguments)]
fn aggregate_expression_borrow_loans(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &psi_checked_trees::statement::TableLocalData,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    expression: ExpressionHandle,
    owner_path_prefix: &[BorrowOwnerSegment],
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    match program.expression_table.expression(expression) {
        psi_checked_trees::expression::ExpressionNode::Call(call) => {
            let field_loans = helper_call_aggregate_borrow_loans(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                local_data,
                call,
                owner_path_prefix,
                loan_trackers,
            );
            if !field_loans.is_empty() {
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
            rebase_borrow_places_through_local_loans(program, place, loan_trackers)
                .into_iter()
                .map(|source| StatementBorrowLoan {
                    owner_symbol: local_data.symbol,
                    owner_name: local_data.name.clone(),
                    owner_path: owner_path_prefix.to_vec(),
                    place: source.place,
                    source_owner_symbol: source.source_owner_symbol,
                    lineage: psi_checked_trees::BorrowLoanLineage::UnretainedDerived,
                    kind: if is_mutably_borrow_carrying_data(program, type_reference) {
                        psi_checked_trees::BorrowAccessKind::Mutable
                    } else {
                        psi_checked_trees::BorrowAccessKind::Read
                    },
                })
                .collect()
        }
        psi_checked_trees::expression::ExpressionNode::Name(_)
        | psi_checked_trees::expression::ExpressionNode::Member(_)
        | psi_checked_trees::expression::ExpressionNode::Indexed(_) => {
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
                &source,
                owner_path_prefix,
                loan_trackers,
            )
        }
        psi_checked_trees::expression::ExpressionNode::Cast(cast) if !cast.form.is_recast() => {
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

fn helper_call_aggregate_borrow_loans(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &psi_checked_trees::statement::TableLocalData,
    call: &psi_checked_trees::expression::TableCallExpression,
    owner_path_prefix: &[BorrowOwnerSegment],
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    let ViewReturnSource::Fields { fields } = call_view_return_source(program, call.target_symbol)
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
                .map(|source| {
                    let mut owner_path = owner_path_prefix.to_vec();
                    owner_path.extend(field.owner_path.iter().copied());
                    StatementBorrowLoan {
                        owner_symbol: local_data.symbol,
                        owner_name: local_data.name.clone(),
                        owner_path,
                        place: source.place,
                        source_owner_symbol: source.source_owner_symbol,
                        lineage: psi_checked_trees::BorrowLoanLineage::UnretainedDerived,
                        kind: field.kind.clone(),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn transferred_aggregate_loans(
    program: &psi_typed_trees::TypedTrees,
    local_data: &psi_checked_trees::statement::TableLocalData,
    source: &accesses::BorrowAccessPlace,
    owner_path_prefix: &[BorrowOwnerSegment],
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

            let mut prefixed_owner_path = owner_path_prefix.to_vec();
            prefixed_owner_path.extend(owner_path);
            Some(StatementBorrowLoan {
                owner_symbol: local_data.symbol,
                owner_name: local_data.name.clone(),
                owner_path: prefixed_owner_path,
                place: loan.place.clone(),
                source_owner_symbol: loan.owner_symbol,
                lineage: psi_checked_trees::BorrowLoanLineage::UnretainedDerived,
                kind: loan.kind.clone(),
            })
        })
        .collect()
}

fn helper_call_borrow_loan_place(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    call: &psi_checked_trees::expression::TableCallExpression,
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

    // The borrow source (self, a named input, or none) is resolved by the same
    // logic the declaration check uses (`borrow::view_link`), so the loan we
    // track here always matches what the elision check accepted. Elision rules
    // 1/3 and stage-2 explicit lifetimes all flow through there.
    match call_view_return_source(program, call.target_symbol) {
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

fn call_view_return_source(
    program: &psi_typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> ViewReturnSource {
    if let Some(target_state) = find_state(program, target_symbol) {
        return resolve_view_return_source(program, target_state);
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target_symbol) {
        return resolve_signature_view_return_source(
            program,
            program.state_signature_parameters(signature),
            signature.return_type,
        );
    }
    for trait_definition in program.traits() {
        if let Some(signature) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
        {
            return resolve_signature_view_return_source(
                program,
                program.state_signature_parameters(signature),
                signature.return_type,
            );
        }
    }
    ViewReturnSource::NotApplicable
}

/// The loan place for a call argument that the called machine's returned view
/// borrows (elision rule 1). Handles arguments that are themselves
/// view-producing calls (`bag.cells.as_mut_slice()`, or a nested one-ref-input
/// machine call) by recursing into the call's own loan place.
fn argument_borrow_loan_place(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    argument: ExpressionHandle,
) -> Option<accesses::BorrowAccessPlace> {
    match program.expression_table.expression(argument) {
        psi_checked_trees::expression::ExpressionNode::Call(inner_call) => {
            helper_call_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                inner_call,
            )
        }
        psi_checked_trees::expression::ExpressionNode::Borrow(inner)
            if matches!(
                program.expression_table.expression(inner.target),
                psi_checked_trees::expression::ExpressionNode::Call(_)
            ) =>
        {
            argument_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                inner.target,
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
    program: &psi_typed_trees::TypedTrees,
    place: accesses::BorrowAccessPlace,
    loan_trackers: &[StateLoanTracker],
) -> Vec<RebasedBorrowPlace> {
    let source_loans: Vec<&StateLoanTracker> = loan_trackers
        .iter()
        .rev()
        .filter(|loan| {
            loan.owner_symbol == place.root_symbol
                && owner_path_matches(program, &loan.owner_path, &place.segments)
        })
        .collect();
    if source_loans.is_empty() {
        return vec![RebasedBorrowPlace {
            place,
            source_owner_symbol: SymbolHandle::invalid(),
            parent_loan: Handle::invalid(),
            parent_lineage_is_retained: false,
        }];
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

            RebasedBorrowPlace {
                place: accesses::BorrowAccessPlace {
                    root_symbol: source_loan.place.root_symbol,
                    segments: rebased_segments,
                },
                source_owner_symbol: source_loan.owner_symbol,
                parent_loan: source_loan.handle,
                parent_lineage_is_retained: source_loan.lineage
                    != psi_checked_trees::BorrowLoanLineage::UnretainedDerived,
            }
        })
        .collect()
}

fn retained_reference_lineage(
    source: &RebasedBorrowPlace,
    rebased: &[RebasedBorrowPlace],
    is_explicit_reborrow: bool,
    force_unretained: bool,
) -> psi_checked_trees::BorrowLoanLineage {
    if force_unretained {
        return psi_checked_trees::BorrowLoanLineage::UnretainedDerived;
    }
    if !source.source_owner_symbol.is_valid() {
        return psi_checked_trees::BorrowLoanLineage::DirectRoot;
    }
    let parent_is_unique = rebased.len() == 1;
    if is_explicit_reborrow
        && source.parent_loan.is_valid()
        && source.parent_lineage_is_retained
        && parent_is_unique
    {
        psi_checked_trees::BorrowLoanLineage::Reborrow {
            parent_loan: source.parent_loan,
        }
    } else {
        psi_checked_trees::BorrowLoanLineage::UnretainedDerived
    }
}
