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
        borrow_call,
        state_mutation_summaries,
    );
    let use_mutable_argument_fallback = summarized_places
        .as_ref()
        .is_none_or(|summarized_places| summarized_places.is_empty());
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
            if access.kind == BorrowAccessKind::Mutable
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
    match statement {
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
    }
}
