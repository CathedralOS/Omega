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
            storage_upper_bounds: None,
            storage_non_negatives: None,
            storage_minimum_lengths: None,
        });
        collected
            .last_mut()
            .expect("state argument facts were just inserted")
    };

    // An ordinary invocation starts with its declared receiver obligations;
    // only a state transition hands off the same receiver's live storage.
    let mut incoming_lengths = if receiver_transition {
        facts.receiver_lengths(program, machine, state)
    } else {
        Vec::new()
    };
    // A transition argument's referent is storage the destination parameter
    // aliases for the whole state, exactly like attached `self`: every label
    // fact spelled under the argument's place re-keys onto the parameter's
    // name (`put(&mut buf)` delivers `buf.i` as the callee's `buf.i`). A `->
    // self` edge maps each parameter to its own continuing storage. Ordinary
    // calls and runtime-selector arguments carry no storage evidence.
    let argument_storages: Vec<(crate::flow::CanonicalPlace, SymbolHandle)> =
        if !receiver_transition {
            Vec::new()
        } else if arguments.is_empty() {
            entry
                .parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .filter_map(|parameter| {
                    Some((
                        crate::flow::canonical_place_from_symbol(parameter.symbol)?,
                        parameter.symbol,
                    ))
                })
                .collect()
        } else {
            entry
                .parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .zip(arguments.iter().copied())
                .filter_map(|(parameter, argument)| {
                    let mut place =
                        crate::flow::canonical_place_from_expression(program, argument)?;
                    if !matches!(place.root, facts::PlaceRoot::Symbol(_)) {
                        return None;
                    }
                    if !place.segments.iter().all(|segment| {
                        matches!(
                            segment,
                            facts::PlaceSegment::Field { .. }
                                | facts::PlaceSegment::Case { .. }
                                | facts::PlaceSegment::FixedIndex { .. }
                        )
                    }) {
                        return None;
                    }
                    place = crate::flow::rebase_exact_local_place(
                        program,
                        state.symbol,
                        facts.statement_index,
                        place,
                        facts
                            .checked_calls
                            .and_then(|context| context.call_frames()),
                    )?;
                    crate::flow::normalize_attached_place_root(
                        program,
                        machine.symbol,
                        state.symbol,
                        &mut place,
                    );
                    place.root = crate::flow::normalized_event_place_root(program, place.root);
                    Some((place, parameter.symbol))
                })
                .collect()
        };
    let storage_rows = facts.storage_sub_places(program, machine, state, &argument_storages);
    let mut incoming_upper_bounds = Vec::new();
    let mut incoming_non_negatives = Vec::new();
    let mut incoming_minimum_lengths = Vec::new();
    for (source_label, expression, destination_place) in &storage_rows {
        let destination_label = facts::canonical_place_label_from_parts(
            program,
            destination_place.root,
            &destination_place.segments,
        );
        if let Some(length) = facts.exact_length(source_label) {
            incoming_lengths.push(crate::checks::ranges::facts::ReceiverLength::at_place(
                *expression,
                destination_place.clone(),
                destination_label.clone(),
                length,
            ));
        }
        if let Some(bound) = facts.proven_index_upper_bound(source_label) {
            incoming_upper_bounds.push((destination_label.clone(), bound));
        }
        if facts.non_negative_is_proven(source_label) {
            incoming_non_negatives.push(destination_label.clone());
        }
        if let Some(minimum) = facts.minimum_length(source_label) {
            incoming_minimum_lengths.push((destination_label, minimum));
        }
    }
    super::merge_storage_rows(
        &mut entry.storage_upper_bounds,
        &Some(incoming_upper_bounds),
        |a, b| a.max(b),
    );
    super::merge_storage_rows(
        &mut entry.storage_minimum_lengths,
        &Some(incoming_minimum_lengths),
        |a, b| a.min(b),
    );
    {
        let incoming = Some(incoming_non_negatives);
        if let Some(existing) = &mut entry.storage_non_negatives {
            existing.retain(|label| incoming.as_ref().is_some_and(|rows| rows.contains(label)));
        } else {
            entry.storage_non_negatives = incoming;
        }
    }
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
                    index: ParameterIndexProofSide::Storage(index.clone()),
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
                    collection: ParameterIndexProofSide::Storage(collection.clone()),
                    index: ParameterIndexProofSide::Parameter(index_parameter),
                }),
        );
    }
    // Argument-referent storage: a proven pair spelled under an argument's
    // place arrives spelled under the destination parameter on either side
    // (`put(&mut buf)` delivers `buf.line`'s proven `buf.i` index as
    // `param.line`'s proven `param.i` index).
    let storage_sides: Vec<(String, ParameterIndexProofSide)> = storage_rows
        .iter()
        .map(|(source_label, _, destination_place)| {
            (
                source_label.clone(),
                ParameterIndexProofSide::Storage(facts::canonical_place_label_from_parts(
                    program,
                    destination_place.root,
                    &destination_place.segments,
                )),
            )
        })
        .collect();
    let parameter_sides: Vec<(String, ParameterIndexProofSide)> = parameter_arguments
        .iter()
        .map(|(position, argument)| {
            (
                program.expression_table.display_name(*argument),
                ParameterIndexProofSide::Parameter(*position),
            )
        })
        .collect();
    for (collection_source, collection_side) in &storage_sides {
        for (index_source, index_side) in storage_sides.iter().chain(parameter_sides.iter()) {
            if facts.index_is_proven(collection_source, index_source) {
                index_proofs.push(ParameterIndexProof {
                    collection: collection_side.clone(),
                    index: index_side.clone(),
                });
            }
        }
    }
    for (index_source, index_side) in &storage_sides {
        for (collection_source, collection_side) in &parameter_sides {
            if facts.index_is_proven(collection_source, index_source) {
                index_proofs.push(ParameterIndexProof {
                    collection: collection_side.clone(),
                    index: index_side.clone(),
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
