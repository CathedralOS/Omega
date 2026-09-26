use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

use super::{
    MergedFact, ParameterFacts, ParameterIndexProof, ParameterIndexProofSide, StateArgumentFacts,
};
use crate::checks::ranges::expressions::{
    ensured_call_result_bounds, expression_indexable_length, expression_integer_value,
};
use crate::checks::ranges::facts::RangeFacts;
use crate::checks::ranges::proofs::unknown_length_index_is_proven;
use language_core::is_receiver_rooted;

pub(super) fn collect_state_argument_facts_for_call(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    target_symbol: SymbolHandle,
    arguments: &[ExpressionHandle],
    receiver_transition: bool,
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
            machine: machine.symbol,
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
                    non_negative: MergedFact::Unseen,
                })
                .collect(),
            index_proofs: Default::default(),
            receiver_lengths: None,
        });
        collected
            .last_mut()
            .expect("state argument facts were just inserted")
    };

    // An ordinary invocation starts with its declared receiver obligations;
    // only a state transition hands off the same receiver's live storage.
    let incoming_lengths = if receiver_transition {
        facts.receiver_lengths(program, machine, state)
    } else {
        Vec::new()
    };
    if let Some(existing) = &mut entry.receiver_lengths {
        existing.retain(|length| {
            incoming_lengths
                .iter()
                .any(|incoming| length.same_extent(incoming))
        });
    } else {
        entry.receiver_lengths = Some(incoming_lengths);
    }

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
        // R4 transport: every proven bound on the argument is sound for the
        // destination parameter, so meet them. A constant bounds exclusively
        // at value+1, a bound name carries its seeded label fact, and a call
        // argument's own `ensures` substitutes the exit proof the callee
        // discharged at every return (`result <= K` bounds this occurrence's
        // result at K+1) — the consumer consults the contract rather than
        // weakening the index admission downstream.
        let ensured = ensured_call_result_bounds(program, argument);
        let argument_label = program.expression_table.display_name(argument);
        let argument_bound = [
            expression_integer_value(program, facts, argument)
                .and_then(|value| (value >= 0).then(|| value.checked_add(1)).flatten()),
            facts.proven_index_upper_bound(&argument_label),
            ensured
                .and_then(|(_, high)| high)
                .and_then(|high| high.checked_add(1)),
        ]
        .into_iter()
        .flatten()
        .min();
        parameter.upper_bound.merge(argument_bound);
        // The same exit proof discharges the lower half for a signed
        // parameter: the argument is non-negative when it folds to a
        // non-negative literal, carries a proven label fact, or the call's
        // ensured conjunct bounds its result `>= 0`. One unproven edge
        // poisons the lane like every other merged fact.
        let non_negative = (expression_integer_value(program, facts, argument)
            .is_some_and(|value| value >= 0)
            || facts.non_negative_is_proven(&argument_label)
            || ensured.and_then(|(low, _)| low).is_some_and(|low| low >= 0))
        .then_some(());
        parameter.non_negative.merge(non_negative);
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
                index_proofs.push(ParameterIndexProof {
                    collection: ParameterIndexProofSide::Parameter(collection_parameter),
                    index: ParameterIndexProofSide::Parameter(index_parameter),
                });
            }
        }
        // A pair can also ride machine storage on the index side: `self.field`
        // names the same place in every state of this machine, so a proven
        // `(argument, self.field)` pair re-keys onto the destination parameter.
        let collection_label = program.expression_table.display_name(collection);
        index_proofs.extend(
            facts
                .proven_index_labels(&collection_label)
                .filter(|index| is_receiver_rooted(index))
                .map(|index| ParameterIndexProof {
                    collection: ParameterIndexProofSide::Parameter(collection_parameter),
                    index: ParameterIndexProofSide::MachineStorage(index.clone()),
                }),
        );
    }
    for (index_parameter, index) in parameter_arguments.iter().copied() {
        // The mirror: `(self.field, argument)` -- machine storage on the
        // collection side (`self.slice[index_arg]`).
        let index_label = program.expression_table.display_name(index);
        index_proofs.extend(
            facts
                .proven_collections_for_index(&index_label)
                .filter(|collection| is_receiver_rooted(collection))
                .map(|collection| ParameterIndexProof {
                    collection: ParameterIndexProofSide::MachineStorage(collection.clone()),
                    index: ParameterIndexProofSide::Parameter(index_parameter),
                }),
        );
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
