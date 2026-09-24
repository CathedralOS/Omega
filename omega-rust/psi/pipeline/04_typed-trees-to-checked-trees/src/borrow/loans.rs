use crate::borrow::view_link::{
    ViewReturnSource, is_borrow_carrying_data, is_mutably_borrow_carrying_data,
    resolve_signature_view_return_source,
};
use crate::semantic::calls::find_state;
use arena::Handle;
use checked_trees::expression::ExpressionHandle;
use checked_trees::name::Identifier;
use checked_trees::statement::StatementNode;
use language_semantics::declaration_selection::CollectionViewOperation;
use symbols::SymbolHandle;

use super::accesses::{self, borrow_access_place};
use super::tracker::{BorrowOwnerSegment, StateLoanTracker};
use aggregate::{BorrowedInitializer, BorrowedInitializerKind, borrowed_initializers};
use owner_paths::{
    owner_path_from_place_segments, owner_path_matches, place_path_matches_owner_prefix,
};

mod aggregate;
mod owner_paths;
mod returned_carriers;
pub(crate) mod types;

use types::{is_reference_type, reference_borrow_access_kind};

#[derive(Clone)]
pub(super) struct StatementBorrowLoan {
    pub(super) owner_symbol: SymbolHandle,
    pub(super) owner_name: Identifier,
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) place: accesses::BorrowAccessPlace,
    pub(super) source_owner_symbol: SymbolHandle,
    pub(super) lineage: checked_trees::BorrowLoanLineage,
    pub(super) kind: checked_trees::BorrowAccessKind,
    /// The loan's retained root was captured from a call result, not from a
    /// borrow expression. It resolves receivers like any direct root, but it
    /// is not borrow ancestry: children formed through it stay derived rather
    /// than claiming `Reborrow` lineage.
    pub(super) call_result: bool,
}

struct RebasedBorrowPlace {
    place: accesses::BorrowAccessPlace,
    source_owner_symbol: SymbolHandle,
    parent_loan: Handle<checked_trees::BorrowLoanFact>,
    parent_lineage_is_retained: bool,
}

/// The initializer expressions that supply the references structurally carried
/// by `type_reference`. Persistent-storage checking uses this same traversal so
/// its static-source exemption cannot disagree with local aggregate loan
/// attribution about which nested fields actually borrow.
pub(crate) fn borrow_initializer_expressions(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    expression: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    borrowed_initializers(program, type_reference, expression, &[], &[])
        .into_iter()
        .map(|initializer| initializer.expression)
        .collect()
}

pub(super) fn statement_borrow_loans(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
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
        StatementNode::RootBinding(_)
        | StatementNode::AssemblyFact(_)
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
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    assignment: &checked_trees::statement::TableAssignment,
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
    let synthetic_local = checked_trees::statement::TableLocalData {
        symbol: local.symbol,
        name: local.name.clone(),
        type_reference: target_type,
        initial_value: assignment.value,
        is_mutable: true,
        type_is_inferred: false,
        relevance: language_core::BindingRelevance::Relevant,
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
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    loan_trackers: &[StateLoanTracker],
    allow_direct_reborrow_lineage: bool,
) -> Vec<StatementBorrowLoan> {
    let Some(local_access) = reference_borrow_access_kind(program, local_data.type_reference)
    else {
        return Vec::new();
    };
    if let Some(mut loans) = returned_carriers::result_loans(
        program,
        state.symbol,
        statement_index,
        machine_symbol,
        local_data,
        local_data.initial_value,
        &[],
        loan_trackers,
    ) {
        returned_carriers::attenuate(&mut loans, &local_access);
        return loans;
    }
    // A match joining borrowed arms lends every arm's exact target for the
    // result's whole live range: the run-time edge decides which source
    // carried the borrow, so each source stays constrained as if that edge
    // ran. Non-borrow arms contribute no loan rather than a fabricated source;
    // result admission elsewhere keeps the selected set uniform.
    if let checked_trees::expression::ExpressionNode::Match(dispatch) = program
        .expression_table
        .expression(local_data.initial_value)
    {
        return selected_arm_borrow_loans(
            program,
            state.symbol,
            statement_index,
            machine_symbol,
            local_data,
            dispatch,
            loan_trackers,
        );
    }
    let indexed_recast_place = literal_indexed_recast_borrow_place(
        program,
        state,
        statement_index,
        machine_symbol,
        local_data,
    );
    let explicit_reborrow_target = direct_reborrow_target(program, local_data.initial_value);
    let is_explicit_reborrow = explicit_reborrow_target.is_some();
    // A call result keeps direct provenance only when its declared view
    // source resolves to storage carrying no live local loan: the callee
    // signature already names the exact parameter or receiver the return
    // borrows, so the loan captures that storage like a direct borrow does.
    // Calls whose signature declares no single direct source -- ambiguous,
    // carrier-field, or view-free results -- and the name-matched slice/view
    // builtins stay derived. A source place that does rebase through a live
    // local loan still stays derived below -- a call transfer never gains
    // reborrow ancestry. Non-borrow recast casts remain deliberately
    // unretained in every case.
    let call_declares_direct_source = match program
        .expression_table
        .expression(local_data.initial_value)
    {
        checked_trees::expression::ExpressionNode::Call(call) => {
            call_declares_direct_view_source(program, call.target_symbol)
        }
        _ => false,
    };
    let force_unretained = matches!(
        program
            .expression_table
            .expression(local_data.initial_value),
        checked_trees::expression::ExpressionNode::Cast(_)
            | checked_trees::expression::ExpressionNode::Call(_)
    ) && !call_declares_direct_source
        && !is_explicit_reborrow;
    let explicit_reborrow_place = explicit_reborrow_target.and_then(|target| {
        whole_place_recast_borrow_place(
            program,
            state.symbol,
            statement_index,
            target,
            machine_symbol,
        )
        .or_else(|| indexed_recast_place.clone())
        .or_else(|| {
            borrow_access_place(
                program,
                state.symbol,
                statement_index,
                target,
                machine_symbol,
            )
        })
    });
    let inferred_place = match program
        .expression_table
        .expression(local_data.initial_value)
    {
        checked_trees::expression::ExpressionNode::Borrow(inner_expression) => {
            let ordinary = || {
                borrow_access_place(
                    program,
                    state.symbol,
                    statement_index,
                    inner_expression.target,
                    machine_symbol,
                )
            };
            // Typed reference-to-reference borrow targets may carry a whole-place
            // recast regardless of the requested child access. Preserve that
            // parent-local identity for shared attenuation just as for exclusive
            // reborrows so the exact lineage can be retained and classified.
            whole_place_recast_borrow_place(
                program,
                state.symbol,
                statement_index,
                inner_expression.target,
                machine_symbol,
            )
            .or_else(|| indexed_recast_place.clone())
            .or_else(ordinary)
        }
        checked_trees::expression::ExpressionNode::Cast(cast) if cast.form.is_recast() => {
            whole_place_recast_borrow_place(
                program,
                state.symbol,
                statement_index,
                local_data.initial_value,
                machine_symbol,
            )
            .or_else(|| indexed_recast_place.clone())
        }
        checked_trees::expression::ExpressionNode::Call(call) => helper_call_borrow_loan_place(
            program,
            state.symbol,
            statement_index,
            machine_symbol,
            call,
        ),
        checked_trees::expression::ExpressionNode::Indexed(_)
        | checked_trees::expression::ExpressionNode::Member(_)
        | checked_trees::expression::ExpressionNode::Name(_) => borrow_access_place(
            program,
            state.symbol,
            statement_index,
            local_data.initial_value,
            machine_symbol,
        ),
        _ => None,
    };
    let Some(place) = explicit_reborrow_place.or(inferred_place) else {
        return Vec::new();
    };

    let is_call_result = matches!(
        program
            .expression_table
            .expression(local_data.initial_value),
        checked_trees::expression::ExpressionNode::Call(_)
    );
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
            kind: local_access.clone(),
            call_result: is_call_result,
        })
        .collect()
}

/// One loan per match arm that forms a borrow. Each arm's loan records the
/// arm's own access kind against the arm's exact target place, rebased
/// through any live local loans the same way a direct `let` initializer is.
/// A match arm is not an explicit reborrow, so direct sources keep
/// `DirectRoot` lineage and rebased sources stay deliberately unretained.
fn selected_arm_borrow_loans(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    dispatch: &checked_trees::expression::TableMatchExpression,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    program
        .expression_table
        .match_arms(dispatch.arms)
        .iter()
        .flat_map(|arm| {
            let checked_trees::expression::ExpressionNode::Borrow(borrow) =
                program.expression_table.expression(arm.value)
            else {
                return Vec::new();
            };
            let kind = match borrow.access {
                language_semantics::ReferenceAccess::Shared => {
                    checked_trees::BorrowAccessKind::Read
                }
                language_semantics::ReferenceAccess::Mutable => {
                    checked_trees::BorrowAccessKind::Mutable
                }
                language_semantics::ReferenceAccess::WriteOnly => {
                    checked_trees::BorrowAccessKind::WriteOnly
                }
            };
            let Some(place) = borrow_access_place(
                program,
                state_symbol,
                statement_index,
                borrow.target,
                machine_symbol,
            ) else {
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
                    lineage: retained_reference_lineage(source, &rebased, false, false),
                    kind: kind.clone(),
                    call_result: false,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn literal_indexed_recast_borrow_place(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
) -> Option<accesses::BorrowAccessPlace> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let footprint =
        validation::validate_literal_indexed_recast_footprint(program, machine, state, local_data)?;
    let mut place = borrow_access_place(
        program,
        state.symbol,
        statement_index,
        footprint.collection(),
        machine_symbol,
    )?;
    place.segments.push(facts::PlaceSegment::FixedRange {
        start: footprint.start(),
        end: footprint.end(),
    });
    Some(place)
}

/// Finds only an explicit borrow possibly wrapped in reference-carrier casts.
/// Typed trees normalize the implicit mutable-to-shared attenuation cast to
/// `Value`, so the stable discriminator here is its reference target and lack
/// of a semantic-domain retag. Numeric/domain casts and helper-returned views
/// remain derived and unretained.
fn direct_reborrow_target(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    match program.expression_table.expression(expression) {
        checked_trees::expression::ExpressionNode::Borrow(inner) => Some(inner.target),
        checked_trees::expression::ExpressionNode::Cast(cast)
            if is_reference_type(program, cast.target_type) && cast.semantic_domain.is_empty() =>
        {
            direct_reborrow_target(program, cast.value)
        }
        _ => None,
    }
}

fn whole_place_recast_borrow_place(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    machine_symbol: SymbolHandle,
) -> Option<accesses::BorrowAccessPlace> {
    let checked_trees::expression::ExpressionNode::Cast(cast) =
        program.expression_table.expression(expression)
    else {
        return None;
    };
    if !cast.form.is_recast()
        || !matches!(
            program.expression_table.expression(cast.value),
            checked_trees::expression::ExpressionNode::Name(_)
                | checked_trees::expression::ExpressionNode::Member(_)
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
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    if matches!(
        program
            .expression_table
            .expression(local_data.initial_value),
        checked_trees::expression::ExpressionNode::Call(_)
            | checked_trees::expression::ExpressionNode::Name(_)
            | checked_trees::expression::ExpressionNode::Member(_)
            | checked_trees::expression::ExpressionNode::Indexed(_)
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
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    initializer: BorrowedInitializer,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    match initializer.kind {
        BorrowedInitializerKind::Reference { is_mutable } => {
            if let Some(mut loans) = returned_carriers::result_loans(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                local_data,
                initializer.expression,
                &initializer.owner_path,
                loan_trackers,
            ) {
                if !is_mutable {
                    returned_carriers::attenuate(
                        &mut loans,
                        &checked_trees::BorrowAccessKind::Read,
                    );
                }
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
            rebase_borrow_places_through_local_loans(program, place, loan_trackers)
                .into_iter()
                .map(|source| StatementBorrowLoan {
                    owner_symbol: local_data.symbol,
                    owner_name: local_data.name.clone(),
                    owner_path: initializer.owner_path.clone(),
                    place: source.place,
                    source_owner_symbol: source.source_owner_symbol,
                    lineage: checked_trees::BorrowLoanLineage::UnretainedDerived,
                    kind: if is_mutable {
                        checked_trees::BorrowAccessKind::Mutable
                    } else {
                        checked_trees::BorrowAccessKind::Read
                    },
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

#[allow(clippy::too_many_arguments)]
fn aggregate_expression_borrow_loans(
    program: &typed_trees::TypedTrees,
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
            if let Some(field_loans) = returned_carriers::result_loans(
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
            rebase_borrow_places_through_local_loans(program, place, loan_trackers)
                .into_iter()
                .map(|source| StatementBorrowLoan {
                    owner_symbol: local_data.symbol,
                    owner_name: local_data.name.clone(),
                    owner_path: owner_path_prefix.to_vec(),
                    place: source.place,
                    source_owner_symbol: source.source_owner_symbol,
                    lineage: checked_trees::BorrowLoanLineage::UnretainedDerived,
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
            if let Some(loans) = returned_carriers::result_loans(
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
                &source,
                owner_path_prefix,
                loan_trackers,
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

fn helper_call_aggregate_borrow_loans(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    call: &checked_trees::expression::TableCallExpression,
    owner_path_prefix: &[BorrowOwnerSegment],
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    let ViewReturnSource::Fields { fields } =
        call_view_return_source_at(program, state_symbol, call)
    else {
        return Vec::new();
    };
    let substitutions = call_view_signature(program, call.target_symbol)
        .and_then(|(_, _, signature)| signature)
        .and_then(|signature| call_site_substitutions(program, state_symbol, signature, call))
        .unwrap_or_default();
    let arguments = program.expression_table.expression_handles(call.arguments);
    let mut carried_arguments = Vec::new();

    fields
        .into_iter()
        .flat_map(|field| {
            let Some(argument) = arguments.get(field.non_self_index).copied() else {
                return Vec::new();
            };
            if !is_reference_type(program, field.source_type) {
                return returned_carriers::argument_loans(
                    program,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                    local_data,
                    argument,
                    &field,
                    owner_path_prefix,
                    loan_trackers,
                    &substitutions,
                    &mut carried_arguments,
                );
            }
            let mut owner_path = owner_path_prefix.to_vec();
            owner_path.extend(field.owner_path.iter().copied());
            if let Some(mut loans) = returned_carriers::result_loans(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                local_data,
                argument,
                &owner_path,
                loan_trackers,
            ) {
                returned_carriers::attenuate(&mut loans, &field.kind);
                return loans;
            }
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
                        lineage: checked_trees::BorrowLoanLineage::UnretainedDerived,
                        kind: field.kind.clone(),
                        call_result: false,
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn transferred_aggregate_loans(
    program: &typed_trees::TypedTrees,
    local_data: &checked_trees::statement::TableLocalData,
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
                lineage: checked_trees::BorrowLoanLineage::UnretainedDerived,
                kind: loan.kind.clone(),
                call_result: false,
            })
        })
        .collect()
}

pub(crate) fn helper_call_borrow_loan_place(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    call: &checked_trees::expression::TableCallExpression,
) -> Option<accesses::BorrowAccessPlace> {
    // A checked call is the same `TableCallExpression` the typed program
    // holds, and none of this function's callers carry `CheckFacts`, so the
    // retained `CollectionView` selection is out of reach here and the call
    // shape answers instead.
    //
    // `Bytes` stays excluded deliberately. `as_slice`/`as_mut_slice`/`as_view`
    // hand back a borrow of the receiver's own storage, which is exactly the
    // loan recorded below. `bytes` reads a view's byte content; whether that
    // result borrows the same storage is a question for the byte-carrier
    // semantics, and nothing here establishes it -- widening it would mint a
    // loan against a place this lane never proved is the source.
    if matches!(
        crate::semantic::calls::collection_view_call(program, call),
        Some(
            CollectionViewOperation::SharedSlice
                | CollectionViewOperation::MutableSlice
                | CollectionViewOperation::TextView
        )
    ) {
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
    // 1/3 and stage-2 explicit lifetimes all flow through there; a generic
    // signature resolves the same relation under the call's closed bindings.
    match call_view_return_source_at(program, state_symbol, call) {
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
    program: &typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> ViewReturnSource {
    let Some((parameters, return_type, _)) = call_view_signature(program, target_symbol) else {
        return ViewReturnSource::NotApplicable;
    };
    resolve_signature_view_return_source(program, parameters, return_type)
}

/// `call_view_return_source` specialized at one call site: a generic
/// signature's frontier only closes under the call's own exact selected
/// callable and argument bindings, which decide which instantiated input
/// each returned view leaf borrows. Calls without closed bindings — concrete
/// states and still-open signatures — keep the declaration-level decision.
fn call_view_return_source_at(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    call: &checked_trees::expression::TableCallExpression,
) -> ViewReturnSource {
    let Some((parameters, return_type, signature)) =
        call_view_signature(program, call.target_symbol)
    else {
        return ViewReturnSource::NotApplicable;
    };
    if let Some(signature) = signature
        && let Some(substitutions) = call_site_substitutions(program, state_symbol, signature, call)
        && !substitutions.is_empty()
    {
        return crate::borrow::view_link::resolve_substituted_view_return_source(
            program,
            parameters,
            return_type,
            &substitutions,
        );
    }
    resolve_signature_view_return_source(program, parameters, return_type)
}

/// The exact type-parameter bindings one static-callable call closes the
/// signature under — the same `closed_static_call_type_bindings` the
/// call-admission gate proves before the call may produce a returned view.
fn call_site_substitutions(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    signature: &typed_trees::signature::StateSignature,
    call: &checked_trees::expression::TableCallExpression,
) -> Option<Vec<(SymbolHandle, typed_trees::types::TypeReferenceHandle)>> {
    let (caller, state) = crate::semantic::calls::find_state_with_machine(program, state_symbol)?;
    validation::closed_static_call_type_bindings(
        program,
        caller,
        state,
        signature,
        &call.machine_arguments,
        program.expression_table.expression_handles(call.arguments),
    )
}

/// True when the call target's own declaration names one exact borrow source
/// for its returned view: the self receiver or a single direct reference
/// parameter. This is the promotion gate for call-result loan provenance --
/// the name-matched slice/view builtins, carrier-field sources, ambiguous
/// signatures, and view-free calls all stay deliberately unretained.
pub(crate) fn call_declares_direct_view_source(
    program: &typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> bool {
    matches!(
        call_view_return_source(program, target_symbol),
        ViewReturnSource::Parameter { .. } | ViewReturnSource::SelfReceiver
    )
}

fn call_view_signature(
    program: &typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<(
    &[typed_trees::signature::StateParameter],
    typed_trees::types::TypeReferenceHandle,
    Option<&typed_trees::signature::StateSignature>,
)> {
    if let Some(target_state) = find_state(program, target_symbol) {
        return Some((
            program.state_parameters(target_state),
            target_state.return_type,
            None,
        ));
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target_symbol) {
        return Some((
            program.state_signature_parameters(signature),
            signature.return_type,
            Some(signature),
        ));
    }
    for trait_definition in program.traits() {
        if let Some(signature) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
        {
            return Some((
                program.state_signature_parameters(signature),
                signature.return_type,
                Some(signature),
            ));
        }
    }
    None
}

/// The loan place for a call argument that the called machine's returned view
/// borrows (elision rule 1). Handles arguments that are themselves
/// view-producing calls (`bag.cells.as_mut_slice()`, or a nested one-ref-input
/// machine call) by recursing into the call's own loan place.
fn argument_borrow_loan_place(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    argument: ExpressionHandle,
) -> Option<accesses::BorrowAccessPlace> {
    match program.expression_table.expression(argument) {
        checked_trees::expression::ExpressionNode::Call(inner_call) => {
            helper_call_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                inner_call,
            )
        }
        checked_trees::expression::ExpressionNode::Borrow(inner)
            if matches!(
                program.expression_table.expression(inner.target),
                checked_trees::expression::ExpressionNode::Call(_)
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
    program: &typed_trees::TypedTrees,
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
                // A call-result root is retained for receiver resolution but
                // carries no borrow ancestry: children formed through it stay
                // derived, so the reborrow resource model never sees more than
                // the suspension shapes it can represent.
                parent_lineage_is_retained: source_loan.lineage
                    != checked_trees::BorrowLoanLineage::UnretainedDerived
                    && !source_loan.call_result,
            }
        })
        .collect()
}

fn retained_reference_lineage(
    source: &RebasedBorrowPlace,
    rebased: &[RebasedBorrowPlace],
    is_explicit_reborrow: bool,
    force_unretained: bool,
) -> checked_trees::BorrowLoanLineage {
    if force_unretained {
        return checked_trees::BorrowLoanLineage::UnretainedDerived;
    }
    if !source.source_owner_symbol.is_valid() {
        return checked_trees::BorrowLoanLineage::DirectRoot;
    }
    let parent_is_unique = rebased.len() == 1;
    if is_explicit_reborrow
        && source.parent_loan.is_valid()
        && source.parent_lineage_is_retained
        && parent_is_unique
    {
        checked_trees::BorrowLoanLineage::Reborrow {
            parent_loan: source.parent_loan,
        }
    } else {
        checked_trees::BorrowLoanLineage::UnretainedDerived
    }
}
