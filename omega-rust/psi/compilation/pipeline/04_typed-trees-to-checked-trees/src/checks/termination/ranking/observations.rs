//! Project admitted source measure metadata through its exact chosen coordinates.
//! This runs at source admission, not in execution selection. It proves no new
//! range or descent; a missing executable vocabulary withholds observation rows.

use super::{
    OrderResolution, RankingOrder, resolve_machine_witness_subjects,
    resolve_machine_witness_view_arguments,
};
use crate::places::place_at;
use checked_trees::{CheckedNaturalRankMeasure, CheckedStateNaturalRank};
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::PrimitiveType;

pub(in crate::checks::termination) fn prepare_natural_observations(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    admitted: &mut checked_trees::CheckedRankArrivalPlan,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CheckedStateNaturalRank>> {
    let states = program.machine_states(machine);
    let root = states.first()?;
    let witness = machine.termination_plan.implementation_witness.as_ref()?;
    let subjects = resolve_machine_witness_subjects(program, machine)?;
    let arguments = resolve_machine_witness_view_arguments(program, machine)?;
    let OrderResolution::Resolved(order) =
        RankingOrder::resolve_witness(program, root, witness, &subjects, &arguments)
    else {
        return None;
    };
    let body_states = admitted.for_body(machine.symbol, machine.states)?.states;
    let relevance = validation::rank_relevance(program, &[machine.symbol])?;
    let coordinate = |expression| {
        let place = place_at(program, root.symbol, 0, expression)?;
        let facts::PlaceRoot::Symbol(parameter) = place.root else {
            return None;
        };
        Some(validation::RankSubjectCoordinate {
            parameter,
            path: place.segments,
        })
    };
    let subject = *subjects.first()?;
    let input = coordinate(subject)?;
    let mut entry = input.clone();
    if let RankingOrder::CustomStructView {
        path, field_symbol, ..
    } = &order
    {
        entry
            .path
            .extend(path.iter().map(|step| facts::PlaceSegment::Field {
                symbol: step.field_symbol,
            }));
        entry.path.push(facts::PlaceSegment::Field {
            symbol: *field_symbol,
        });
        // The resolved measure owns the declaration path; the ordinary typed
        // coordinate query authenticates its composition with this subject.
        validation::ranking_coordinate_type(program, root, &entry)?;
    }
    let mut observations = Vec::new();
    for state in states
        .iter()
        .filter(|state| relevance.requires_rank(state.symbol))
    {
        let mut rows = admitted
            .states
            .span(body_states)?
            .iter()
            .filter(|row| row.state == state.symbol);
        let row = rows.next()?;
        if rows.next().is_some() {
            return None;
        }
        let selected = validation::RankStateBindings::from_checked(admitted, row)?;
        let destination = selected.destination(&entry)?;
        let parameter_position = typed_trees::signature::state_parameter_position(
            program.state_parameters(state),
            destination.parameter,
        )?;
        let primitive = || {
            program.primitive_type_reference(validation::ranking_coordinate_type(
                program,
                state,
                &destination,
            )?)
        };
        let measure = match (&order, subjects.as_slice()) {
            (
                RankingOrder::CustomNatDescending { measure }
                | RankingOrder::CustomScalarView { measure, .. }
                | RankingOrder::CustomStructView { measure, .. },
                [_],
            ) => {
                let declaration = program
                    .measures()
                    .iter()
                    .find(|definition| definition.symbol == *measure)?;
                let formal = declaration.parameter.as_ref()?;
                let inputs = selected.declaration_bindings(formal.symbol, &input);
                if inputs.is_empty() {
                    return None;
                }
                // Named/aggregate prefixes need no selector receipt. Only an
                // indexed destination asks the entry projection owner for its
                // exact state-local bounds, before executable observation.
                let values = if inputs.iter().any(|input| has_index(&input.destination)) {
                    let paths = inputs
                        .iter()
                        .map(|input| input.destination.path.clone())
                        .collect::<Vec<_>>();
                    Some(super::super::rank_inputs::at_entry(
                        program, machine, state, frames, &paths,
                    )?)
                } else {
                    None
                };
                let bindings = inputs
                    .into_iter()
                    .map(|binding| {
                        // The entry is a measure-formal template, while its
                        // destination is an executable state-local observation.
                        let entry = retain_coordinate(admitted, binding.entry, Vec::new());
                        let destination = retain_observed_coordinate(
                            admitted,
                            binding.destination,
                            values.as_ref(),
                        )?;
                        Some(checked_trees::CheckedRankCoordinateBinding {
                            entry,
                            destination,
                            ..Default::default()
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let bindings = admitted.bindings.insert_many(bindings);
                CheckedNaturalRankMeasure::DeclaredMeasure {
                    measure: *measure,
                    bindings,
                    primitive_type: program.primitive_type_reference(declaration.return_type)?,
                }
            }
            (RankingOrder::SliceLength, [_]) => {
                let path = admitted.path_segments.insert_many(destination.path.clone());
                let coordinate =
                    admitted
                        .coordinates
                        .insert(checked_trees::CheckedRankCoordinate {
                            parameter: destination.parameter,
                            path,
                            entry_indexes: Default::default(),
                        });
                CheckedNaturalRankMeasure::SequenceLength { coordinate }
            }
            (RankingOrder::NatDescending, [_]) => {
                let primitive_type = primitive()?;
                if !primitive_type.is_fixed_integer() {
                    return None;
                }
                let inputs = if has_index(&destination) {
                    Some(super::super::rank_inputs::at_entry(
                        program,
                        machine,
                        state,
                        frames,
                        std::slice::from_ref(&destination.path),
                    )?)
                } else {
                    None
                };
                let coordinate =
                    retain_observed_coordinate(admitted, destination.clone(), inputs.as_ref())?;
                CheckedNaturalRankMeasure::IntegerValue {
                    coordinate,
                    primitive_type,
                }
            }
            (RankingOrder::BoundedDistance, [_, upper]) => distance_measure(primitive()?, *upper)?,
            (RankingOrder::IncreasingTo(upper), [_]) => distance_measure(primitive()?, *upper)?,
            _ => return None,
        };
        observations.push(CheckedStateNaturalRank {
            function: checked_trees::CheckedNaturalRankFunction::Identity,
            state: state.symbol,
            parameter: destination.parameter,
            parameter_position: u32::try_from(parameter_position).ok()?,
            measure,
            observation: Default::default(),
        });
    }
    Some(observations)
}

fn distance_measure(
    primitive_type: PrimitiveType,
    upper: ExpressionHandle,
) -> Option<CheckedNaturalRankMeasure> {
    if !matches!(
        primitive_type,
        typed_trees::unsigned_integer_primitive_pattern!()
    ) {
        return None;
    }
    Some(CheckedNaturalRankMeasure::BoundedDistance {
        primitive_type,
        upper,
    })
}

fn has_index(coordinate: &validation::RankSubjectCoordinate) -> bool {
    coordinate
        .path
        .iter()
        .any(|step| matches!(step, facts::PlaceSegment::Index { .. }))
}

fn retain_observed_coordinate(
    admitted: &mut checked_trees::CheckedRankArrivalPlan,
    coordinate: validation::RankSubjectCoordinate,
    inputs: Option<&super::super::rank_inputs::EntryInputs<'_>>,
) -> Option<checked_trees::CheckedRankCoordinateHandle> {
    if !has_index(&coordinate) {
        return Some(retain_coordinate(admitted, coordinate, Vec::new()));
    }
    let prepared = inputs?;
    // Copied selector expressions need not share AST handles. Retain the
    // captured path with its own bounds; never retarget an index receipt.
    let captured = prepared.coordinates.iter().find(|captured| {
        captured.parameter == coordinate.parameter
            && captured.observation == validation::RankPremiseObservation::Value
            && prepared
                .values
                .same_scalar_projection(&captured.path, &coordinate.path)
    })?;
    Some(retain_coordinate(
        admitted,
        validation::RankSubjectCoordinate {
            parameter: captured.parameter,
            path: captured.path.clone(),
        },
        captured.entry_indexes.clone(),
    ))
}

fn retain_coordinate(
    admitted: &mut checked_trees::CheckedRankArrivalPlan,
    coordinate: validation::RankSubjectCoordinate,
    entry_indexes: Vec<checked_trees::CheckedEntryIndexBound>,
) -> checked_trees::CheckedRankCoordinateHandle {
    let path = admitted.path_segments.insert_many(coordinate.path);
    let entry_indexes = admitted.entry_indexes.insert_many(entry_indexes);
    admitted
        .coordinates
        .insert(checked_trees::CheckedRankCoordinate {
            parameter: coordinate.parameter,
            path,
            entry_indexes,
        })
}
