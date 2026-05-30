use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::ExpressionHandle;
use omega_typed_trees::machine::Machine;

use super::{MergedFact, ParameterFacts, StateArgumentFacts};
use crate::checks::ranges::expressions::{expression_indexable_length, expression_integer_value};
use crate::checks::ranges::facts::RangeFacts;
use crate::checks::ranges::proofs::unknown_length_index_is_proven;

pub(super) fn collect_state_argument_facts_for_call(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    facts: &RangeFacts<'_>,
    target_symbol: SymbolHandle,
    target_name: Option<&omega_typed_trees::name::Identifier>,
    arguments: &[ExpressionHandle],
    collected: &mut Vec<StateArgumentFacts>,
) {
    let Some(target_state) = program.machine_states(machine).iter().find(|state| {
        (target_symbol.is_valid() && state.symbol == target_symbol)
            || target_name.is_some_and(|target_name| state.name == *target_name)
    }) else {
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
                    integer: MergedFact::Unseen,
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
        parameter
            .length
            .merge(expression_indexable_length(program, facts, argument));
        parameter
            .integer
            .merge(expression_integer_value(program, facts, argument));
    }

    let mut index_proofs = Vec::new();
    for (collection_parameter, collection) in parameter_arguments.iter().copied() {
        for (index_parameter, index) in parameter_arguments.iter().copied() {
            if collection_parameter == index_parameter {
                continue;
            }
            if argument_is_proven_index_for_collection(program, facts, collection, index) {
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
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
) -> bool {
    if let Some(length) = expression_indexable_length(program, facts, collection) {
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
