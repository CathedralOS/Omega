//! Project admitted source measure metadata through its exact chosen coordinates.
//! This runs at source admission, not in execution selection. It proves no new
//! range or descent; a missing executable vocabulary withholds observation rows.

use super::{
    OrderResolution, RankingOrder, resolve_machine_witness_subjects,
    resolve_machine_witness_view_arguments,
};
use crate::places::place_at;
use checked_trees::{CheckedNaturalRankMeasure as RankMeasure, CheckedStateNaturalRank};
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
    let entry = input.clone();
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
        let application_facts = row.application;
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
                | RankingOrder::CustomScalarView { measure, .. },
                [_],
            ) => {
                let declaration = program
                    .measures()
                    .iter()
                    .find(|definition| definition.symbol == *measure)?;
                let shape = validation::declared_input_shape(program, declaration)?;
                let inputs = shape
                    .reads
                    .iter()
                    .map(|read| {
                        let entry = validation::RankSubjectCoordinate {
                            parameter: input.parameter,
                            path: input
                                .path
                                .iter()
                                .chain(&read.coordinate.path)
                                .cloned()
                                .collect(),
                        };
                        Some(validation::RankCoordinateBinding {
                            entry: read.coordinate.clone(),
                            destination: selected.destination(&entry)?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
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
                let formation = if application_facts.definition_body.is_valid() {
                    if application_facts.measure != *measure
                        || program
                            .expression_table
                            .expression_handles(declaration.body)
                            != [application_facts.definition_body]
                    {
                        return None;
                    }
                    let whole_input = retain_coordinate(admitted, destination.clone(), Vec::new());
                    let owned_copy = declaration.parameter.as_ref().and_then(|formal| {
                        super::super::application_inputs::owned_copy(
                            program,
                            admitted,
                            state,
                            formal.symbol,
                            whole_input,
                        )
                    });
                    admitted.application_formations.insert(
                        checked_trees::CheckedRankApplicationFormation {
                            machine: machine.symbol,
                            machine_body: machine.states,
                            state: state.symbol,
                            application: application_facts,
                            bindings,
                            owned_copy,
                        },
                    )
                } else {
                    Default::default()
                };
                RankMeasure::DeclaredMeasure {
                    formation,
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
                RankMeasure::SequenceLength { coordinate }
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
                RankMeasure::IntegerValue {
                    coordinate,
                    primitive_type,
                }
            }
            (RankingOrder::BoundedDistance, [lower, upper]) => {
                exact_distance_measure(program, admitted, root.symbol, *lower, *upper)?
            }
            (RankingOrder::IncreasingTo(upper), [_]) => {
                increasing_to_measure(primitive()?, *upper)?
            }
            _ => return None,
        };
        let components = admitted.observation_components.insert_many([
            checked_trees::CheckedIntegerRankObservation {
                function: checked_trees::CheckedNaturalRankFunction::Identity,
                parameter: destination.parameter,
                parameter_position: u32::try_from(parameter_position).ok()?,
                measure,
                observation: Default::default(),
            },
        ]);
        observations.push(CheckedStateNaturalRank {
            state: state.symbol,
            components,
        });
    }
    Some(observations)
}

fn exact_distance_measure(
    program: &dyn validation::CheckingInput,
    admitted: &mut checked_trees::CheckedRankArrivalPlan,
    state: symbols::SymbolHandle,
    lower: ExpressionHandle,
    upper: ExpressionHandle,
) -> Option<RankMeasure> {
    let primitive = |expression| {
        program.primitive_type_reference(
            validation::mathematical_applications::operand_type_reference(program, expression)?,
        )
    };
    let lower_type = primitive(lower)?;
    let upper_type = primitive(upper)?;
    if !matches!(
        lower_type,
        typed_trees::unsigned_integer_primitive_pattern!()
    ) || !upper_type.is_fixed_integer()
    {
        return None;
    }
    let lower = admitted
        .rank_operands
        .insert(checked_trees::CheckedRankOperand::Expression {
            state,
            expression: lower,
            primitive_type: lower_type,
        });
    let upper = admitted
        .rank_operands
        .insert(checked_trees::CheckedRankOperand::Expression {
            state,
            expression: upper,
            primitive_type: upper_type,
        });
    Some(RankMeasure::Distance { lower, upper })
}

fn increasing_to_measure(
    primitive_type: PrimitiveType,
    upper: ExpressionHandle,
) -> Option<RankMeasure> {
    if !matches!(
        primitive_type,
        typed_trees::unsigned_integer_primitive_pattern!()
    ) {
        return None;
    }
    Some(RankMeasure::IncreasingTo {
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
