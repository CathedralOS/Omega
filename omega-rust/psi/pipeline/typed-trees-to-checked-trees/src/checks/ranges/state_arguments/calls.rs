use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

use super::{MergedFact, ParameterFacts, StateArgumentFacts};
use crate::checks::ranges::expressions::{expression_indexable_length, expression_integer_value};
use crate::checks::ranges::facts::RangeFacts;
use crate::checks::ranges::proofs::unknown_length_index_is_proven;

pub(super) fn collect_state_argument_facts_for_call(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    target_symbol: SymbolHandle,
    arguments: &[ExpressionHandle],
    collected: &mut Vec<StateArgumentFacts>,
) {
    let Some(target_state) = program
        .machine_states(machine)
        .iter()
        .find(|state| target_symbol.is_valid() && state.symbol == target_symbol)
    else {
        return;
    };

    let entry = collected
        .iter_mut()
        .find(|entry| entry.state == target_state.symbol);
    let entry = if let Some(entry) = entry {
        entry
    } else {
        collected.push(StateArgumentFacts {
            state: target_state.symbol,
            parameters: program
                .state_parameters(target_state)
                .iter()
                .map(|parameter| ParameterFacts {
                    symbol: parameter.symbol,
                    name: parameter.name.to_string(),
                    is_self: parameter.is_self,
                    length: MergedFact::Unseen,
                    minimum_length: super::MergedBound::Unseen,
                    integer: MergedFact::Unseen,
                    upper_bound: super::MergedBound::Unseen,
                })
                .collect(),
            index_proofs: Default::default(),
        });
        collected
            .last_mut()
            .expect("state argument facts were just inserted")
    };

    let parameter_arguments: Vec<(usize, ExpressionHandle)> = entry
        .parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| !parameter.is_self)
        .zip(arguments.iter().copied())
        .map(|((index, _), argument)| (index, argument))
        .collect();

    for (parameter, argument) in entry
        .parameters
        .iter_mut()
        .filter(|parameter| !parameter.is_self)
        .zip(arguments.iter().copied())
    {
        parameter.length.merge(expression_indexable_length(
            program, machine, state, facts, argument,
        ));
        parameter
            .integer
            .merge(expression_integer_value(program, facts, argument));
        let minimum_length = facts
            .minimum_length(&program.expression_table.display_name(argument))
            .or_else(|| {
                expression_indexable_length(program, machine, state, facts, argument)
                    .and_then(|length| i64::try_from(length).ok())
            });
        parameter.minimum_length.merge_lower(minimum_length);
        // R4 transport: a constant argument bounds exclusively at value+1;
        // otherwise the argument's own proven upper bound (ensures-seeded
        // or guard-seeded) carries over by display name.
        let argument_bound = expression_integer_value(program, facts, argument)
            .and_then(|value| (value >= 0).then(|| value.checked_add(1)).flatten())
            .or_else(|| {
                facts.proven_index_upper_bound(&program.expression_table.display_name(argument))
            });
        parameter.upper_bound.merge(argument_bound);
    }

    let mut index_proofs = Vec::new();
    for (collection_parameter, collection) in parameter_arguments.iter().copied() {
        for (index_parameter, index) in parameter_arguments.iter().copied() {
            if collection_parameter == index_parameter {
                continue;
            }
            if argument_is_proven_index_for_collection(
                program, machine, state, facts, collection, index,
            ) {
                index_proofs.push(super::ParameterIndexProof {
                    collection_parameter,
                    index_parameter,
                });
            }
        }
    }
    entry.index_proofs.merge(index_proofs);
}

fn argument_is_proven_index_for_collection(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
) -> bool {
    if let Some(length) = expression_indexable_length(program, machine, state, facts, collection) {
        if let Some(index_value) = expression_integer_value(program, facts, index) {
            return index_value >= 0
                && usize::try_from(index_value).is_ok_and(|index| index < length);
        }

        let collection_label = program.expression_table.display_name(collection);
        let index_label = program.expression_table.display_name(index);
        return facts.index_is_proven(&collection_label, &index_label)
            || facts.index_upper_bound_is_proven(&index_label, length);
    }

    unknown_length_index_is_proven(program, facts, collection, index)
}
