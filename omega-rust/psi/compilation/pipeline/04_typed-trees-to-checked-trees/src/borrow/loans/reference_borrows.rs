//! reference borrows slice of loans.

use super::super::accesses::{self, borrow_access_place};
use super::super::tracker::StateLoanTracker;
use super::StatementBorrowLoan;
use super::call_views::*;
use super::types::{is_reference_type, reference_borrow_access_kind};
use crate::borrow::view_link::is_borrow_carrying_data;
use checked_trees::expression::ExpressionHandle;
use symbols::SymbolHandle;

/// Whether a non-reference-typed `let` binds a view anyway: an immutable
/// element-spelling subslice binding (`let v: u8 = arr[a..b]`) binds the
/// view its initializer's own result type names, lending the range
/// read-only exactly as its `&[u8]` spelling does.
pub(super) fn element_view_local_binding(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
) -> bool {
    if local_data.is_mutable {
        return false;
    }
    let Some(machine) = program.machine(machine_symbol) else {
        return false;
    };
    validation::expression_result_type_reference(program, machine, state, local_data.initial_value)
        .and_then(|result| validation::slice_view_shell(program, result, &[]))
        .is_some()
}

pub(super) fn reference_local_borrow_loans(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    let Some(local_access) = reference_borrow_access_kind(program, local_data.type_reference)
        .or_else(|| {
            element_view_local_binding(program, state, machine_symbol, local_data)
                .then_some(checked_trees::BorrowAccessKind::Read)
        })
    else {
        return Vec::new();
    };
    if let Some(mut loans) = super::returned_carriers::result_loans(
        program,
        state.symbol,
        statement_index,
        machine_symbol,
        local_data,
        local_data.initial_value,
        &[],
        loan_trackers,
    ) {
        super::returned_carriers::attenuate(&mut loans, &local_access);
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
        // An arm can forward a name, project a descriptor, call a helper or
        // form a new borrow. Syntax does not select another provenance rule.
        return program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .flat_map(|arm| {
                let selected_value = checked_trees::statement::TableLocalData {
                    initial_value: arm.value,
                    ..local_data.clone()
                };
                reference_local_borrow_loans(
                    program,
                    state,
                    statement_index,
                    machine_symbol,
                    &selected_value,
                    loan_trackers,
                )
            })
            .collect();
    }
    let indexed_recast_place = literal_indexed_recast_borrow_place(
        program,
        state,
        statement_index,
        machine_symbol,
        local_data,
    );
    let runtime_indexed_recast_place = runtime_indexed_recast_borrow_place(
        program,
        state,
        statement_index,
        machine_symbol,
        local_data,
    );
    let explicit_reborrow_target = direct_reborrow_target(program, local_data.initial_value);
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
    let explicit_reborrow_place = explicit_reborrow_target.and_then(|target| {
        whole_place_recast_borrow_place(
            program,
            state.symbol,
            statement_index,
            target,
            machine_symbol,
        )
        .or_else(|| indexed_recast_place.clone())
        .or_else(|| runtime_indexed_recast_place.clone())
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
            .or_else(|| runtime_indexed_recast_place.clone())
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
            .or_else(|| runtime_indexed_recast_place.clone())
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
        | checked_trees::expression::ExpressionNode::Name(_) => {
            // A range-indexed value is itself a borrowed window and always
            // installs the backing loan. Every other place spelling installs
            // its loans only when the place's own declared type carries them:
            // storing a scalar's bits lends nothing, so no loan roots at a
            // non-borrow source place.
            let windowed = matches!(
                program
                    .expression_table
                    .expression(local_data.initial_value),
                checked_trees::expression::ExpressionNode::Indexed(indexed)
                    if matches!(
                        program.expression_table.expression(indexed.index),
                        checked_trees::expression::ExpressionNode::Range(_)
                    )
            );
            let value_carries_borrow = windowed
                || validation::expression_type_reference_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    local_data.initial_value,
                )
                .is_some_and(|value_type| {
                    is_reference_type(program, value_type)
                        || is_borrow_carrying_data(program, value_type)
                });
            if value_carries_borrow {
                borrow_access_place(
                    program,
                    state.symbol,
                    statement_index,
                    local_data.initial_value,
                    machine_symbol,
                )
            } else {
                None
            }
        }
        _ => None,
    };
    let Some(place) = explicit_reborrow_place.or(inferred_place) else {
        return Vec::new();
    };

    let is_call_result = !is_placed_formation(program, local_data.initial_value)
        && matches!(
            program
                .expression_table
                .expression(local_data.initial_value),
            checked_trees::expression::ExpressionNode::Call(_)
        );
    let rebased = rebase_borrow_places_through_local_loans(program, place, loan_trackers);
    rebased
        .iter()
        .map(|source| StatementBorrowLoan {
            formation_expression: local_data.initial_value,
            owner_symbol: local_data.symbol,
            storage_owner: facts::PlaceRoot::Symbol(local_data.symbol),
            owner_name: local_data.name.clone(),
            owner_path: Vec::new(),
            place: source.place.clone(),
            retained_operand: source.retained_operand,
            source_owner_symbol: source.source_owner_symbol,
            supplying_loan: Default::default(),
            lineage: retained_reference_lineage(
                program,
                local_data.initial_value,
                source,
                &rebased,
            ),
            kind: local_access.clone(),
            call_result: is_call_result,
        })
        .collect()
}

pub(super) fn literal_indexed_recast_borrow_place(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
) -> Option<accesses::BorrowAccessPlace> {
    let (machine, referee, access) =
        indexed_recast_loan_parts(program, machine_symbol, local_data)?;
    let footprint = validation::validate_literal_indexed_recast_footprint(
        program,
        machine,
        state,
        statement_index,
        local_data.initial_value,
        referee,
        access,
    )?;
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

/// The runtime-indexed sibling of `literal_indexed_recast_borrow_place`:
/// `validate_runtime_indexed_recast_source` replays the recast judgment and
/// bounds the offset through the interior byte-region walk. Fixed targets
/// keep conservative `Index` coverage; an unsized suffix retains its entire
/// byte collection because one selected byte does not cover the suffix.
/// Unadmitted recast sources mint no loan, restoring the fence the
/// broader recast vocabulary kept before this arm existed.
pub(super) fn runtime_indexed_recast_borrow_place(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
) -> Option<accesses::BorrowAccessPlace> {
    let (machine, referee, access) =
        indexed_recast_loan_parts(program, machine_symbol, local_data)?;
    let source = validation::validate_runtime_indexed_recast_source(
        program,
        machine,
        state,
        statement_index,
        local_data.initial_value,
        referee,
        access,
    )?;
    borrow_access_place(
        program,
        state.symbol,
        statement_index,
        source,
        machine_symbol,
    )
}

/// Finds only an explicit borrow possibly wrapped in reference-carrier casts.
/// Typed trees normalize the implicit mutable-to-shared attenuation cast to
/// `Value`, so the stable discriminator here is its reference target and lack
/// of a semantic-domain retag. Numeric/domain casts and helper-returned views
/// remain derived and unretained.
pub(super) fn direct_reborrow_target(
    program: &dyn ::validation::CheckingInput,
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

pub(super) fn whole_place_recast_borrow_place(
    program: &dyn ::validation::CheckingInput,
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
    // and lifetime. Indexed sources stay with the dedicated arms instead:
    // literal indices keep the exact footprint from
    // `literal_indexed_recast_borrow_place`, and bounded runtime indices mint
    // through `runtime_indexed_recast_borrow_place` once the recast judgment
    // admits them.
    borrow_access_place(
        program,
        state_symbol,
        statement_index,
        cast.value,
        machine_symbol,
    )
}

/// The sealed placed factory forms a borrow from its captured source; it does
/// not return a descriptor whose parent was lost at an ordinary call boundary.
pub(super) fn is_placed_formation(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    validation::reference_formation_source(program, expression).is_some_and(|formation| {
        formation.interpretation == validation::ReferenceFormationInterpretation::Placed
    })
}
