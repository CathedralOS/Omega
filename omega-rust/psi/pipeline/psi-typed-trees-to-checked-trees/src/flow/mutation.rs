use super::*;
use crate::lookup::expression_root_symbol;
mod receiver;
mod summary;

pub(crate) use receiver::{
    call_may_mutate_contract_state, call_receiver_is_mutable, call_receiver_mutated_place,
    canonical_receiver_place_for_call_site,
};
pub(crate) use summary::StateMutationSummaryCache;
use summary::instantiate_known_call_mutation_summary_places;

pub(crate) fn call_mutated_places(
    program: &psi_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &mut StateMutationSummaryCache,
) -> Vec<CanonicalPlace> {
    let summarized_places = instantiate_known_call_mutation_summary_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
    );
    let use_mutable_argument_fallback = summarized_places.is_none();
    let known_target_summary = summarized_places.is_some();
    let mut places = Vec::new();

    if let Some(summarized_places) = summarized_places {
        for place in summarized_places {
            if !places.contains(&place) {
                places.push(place);
            }
        }
    }

    if use_mutable_argument_fallback {
        for access in borrow.argument_accesses.span_or_empty(borrow_call.accesses) {
            if access.kind.is_exclusive()
                && let Some(mut place) = canonical_place_from_symbol(access.root_symbol)
            {
                place.extend_segments(borrow.access_segments.span_or_empty(access.segments));
                if !places.contains(&place) {
                    places.push(place);
                }
            }
        }

        if let Some(call_site) = find_call_site(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        ) && let Some(target_state) = find_state(program, borrow_call.target_symbol)
        {
            let mut argument_index = 0usize;
            for parameter in program.state_parameters(target_state) {
                if parameter.is_self {
                    continue;
                }

                let argument = call_site_argument_expressions(program, &call_site)
                    .get(argument_index)
                    .copied();
                argument_index = argument_index.saturating_add(1);

                if !parameter.is_mutable {
                    continue;
                }

                if let Some(argument) = argument
                    && let Some(place) = canonical_place_from_expression_in_state(
                        program,
                        caller_state_symbol,
                        borrow_call.statement_index,
                        argument,
                    )
                    && !places.contains(&place)
                {
                    places.push(place);
                }
            }
        }
    }

    if !known_target_summary
        && borrow_call.has_receiver
        && call_receiver_is_mutable(program, borrow, borrow_call)
        && let Some(place) = call_receiver_mutated_place(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        )
        && !places.contains(&place)
    {
        places.push(place);
    }

    places
}

pub(crate) fn statement_mutated_place(
    program: &psi_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) -> Option<CanonicalPlace> {
    let mut place = match statement {
        StatementNode::Assignment(assignment) => canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            assignment.target,
        )
        .or_else(|| {
            expression_root_symbol(assignment.target, &program.expression_table, machine_symbol)
                .and_then(canonical_place_from_symbol)
        }),
        _ => None,
    }?;
    normalize_write_only_range_place(program, state_symbol, &mut place);
    Some(place)
}

fn normalize_write_only_range_place(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    place: &mut CanonicalPlace,
) {
    // Keep ordinary borrow selectors expression-backed for certificate replay.
    // Only an admitted write-only mutation may collapse immutable copy bounds
    // into the exact caller-visible range footprint.
    let psi_facts::PlaceRoot::Symbol(root_symbol) = place.root else {
        return;
    };
    let Some(state) = find_state(program, state_symbol) else {
        return;
    };
    let root_is_write_only = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == root_symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local) if local.symbol == root_symbol => {
                        Some(local.type_reference)
                    }
                    _ => None,
                })
        })
        .is_some_and(|type_reference| {
            matches!(
                program.type_reference_table.type_reference(type_reference),
                psi_typed_trees::types::TypeReferenceNode::Reference {
                    access: psi_language_semantics::ReferenceAccess::WriteOnly,
                    ..
                }
            )
        });
    if !root_is_write_only {
        return;
    }

    for segment in &mut place.segments {
        let psi_facts::PlaceSegment::Index { expression } = *segment else {
            continue;
        };
        let ExpressionNode::Range(range) = program.expression_table.expression(expression) else {
            continue;
        };
        let start = if range.start.is_valid() {
            psi_validation::normalize_immutable_integer_bound_to_usize(program, range.start)
        } else {
            Some(0)
        };
        let end = if !range.end.is_valid() {
            None
        } else {
            psi_validation::normalize_immutable_integer_bound_to_usize(program, range.end).and_then(
                |end| {
                    if range.end_inclusive {
                        end.checked_add(1)
                    } else {
                        Some(end)
                    }
                },
            )
        };
        if let (Some(start), Some(end)) = (start, end) {
            *segment = psi_facts::PlaceSegment::FixedRange { start, end };
        }
    }
}
