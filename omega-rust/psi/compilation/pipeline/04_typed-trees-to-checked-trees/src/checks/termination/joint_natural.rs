//! Natural call ranks belong to a complete invocation component. Private
//! participants need an exact carried coordinate, not a fabricated declaration
//! witness. Search declaration identities and judge every actual edge under its
//! own completed prefix; only the full weak-edge DAG establishes termination.

use super::order::{OrderResolution, RankingOrder};
use crate::checks::current_scalar_values::CurrentScalarValues;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::PrimitiveType;
use validation::{ScopedArithmeticValue, ValidatedRuntimeNaturalComponent};

mod ranges;

struct Coordinate {
    parameter: SymbolHandle,
    path: Vec<facts::PlaceSegment>,
    entry_value: ScopedArithmeticValue,
    position: usize,
    primitive: PrimitiveType,
    observation: validation::RankPremiseObservation,
}

struct Edge<'program> {
    source: usize,
    destination: usize,
    statement: usize,
    arguments: &'program [ExpressionHandle],
    values: CurrentScalarValues<'program>,
}

/// Select a natural observation, then judge its exact actual at every call.
/// An integer and a descriptor extent use the same component/edge owner; only
/// reading the carrier differs. Authored ranges additionally owe independent
/// entry formation. Ordinary call checking supplies their exact premises at
/// every invocation; interval endpoints do not parameterize this Nat₀ relation.
/// Private inference supplies no ambient entry facts.
pub(super) fn select(
    program: &dyn validation::CheckingInput,
    component: &[usize],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Result<ValidatedRuntimeNaturalComponent, &'static str>> {
    let machines = component
        .iter()
        .map(|position| &program.machines()[*position])
        .collect::<Vec<_>>();
    // This owner infers private coordinates missing from the declarations.
    // A complete authored component goes directly to the full view owner,
    // which retains its richer result-equation and arithmetic evidence.
    // This is selection by proof obligation, never fallback after a failed proof.
    if machines
        .iter()
        .all(|machine| machine.termination_plan.implementation_witness.is_some())
    {
        return None;
    }
    let mut candidates = Vec::new();
    let mut selected_observation = None;
    for machine in &machines {
        // Source declarations have one entry. Legacy multi-state arrival
        // telescopes remain with the ordinary full-coordinate owner.
        let [entry] = program.machine_states(machine) else {
            return None;
        };
        let Some(witness) = machine.termination_plan.implementation_witness.as_ref() else {
            candidates.push(Vec::new());
            continue;
        };
        let subjects = typed_trees::ranking::resolve_machine_witness_subjects(program, machine)?;
        let arguments =
            typed_trees::ranking::resolve_machine_witness_view_arguments(program, machine)?;
        let observation =
            match RankingOrder::resolve_witness(program, entry, witness, &subjects, &arguments) {
                OrderResolution::Resolved(RankingOrder::NatDescending) => {
                    validation::RankPremiseObservation::Value
                }
                OrderResolution::Resolved(RankingOrder::SliceLength) => {
                    validation::RankPremiseObservation::Length
                }
                _ => return None,
            };
        let [subject] = subjects.as_slice() else {
            return None;
        };
        let place = validation::canonical_place_from_expression_in_state(
            program,
            entry.symbol,
            0,
            *subject,
        )?;
        let parameter = place.root.as_symbol()?;
        let selected = coordinates(program, machine, entry, observation, frames)?
            .into_iter()
            .find(|coordinate| {
                coordinate.parameter == parameter && coordinate.path == place.segments
            })?;
        let carrier = selected.primitive;
        if selected_observation.is_some_and(|previous| previous != (observation, carrier)) {
            return None;
        }
        selected_observation = Some((observation, carrier));
        if let Err(reason) = ranges::entry(program, machine, *subject, observation) {
            return Some(Err(reason));
        }
        candidates.push(vec![selected]);
    }
    let (observation, primitive) = selected_observation?;
    for (machine, candidates) in machines.iter().zip(&mut candidates) {
        if !candidates.is_empty() {
            continue;
        }
        // Private correspondence changes neither public schemas nor source
        // signatures. Only exact runtime leaves in this invocation participate.
        if machine.is_public {
            return None;
        }
        let entry = &program.machine_states(machine)[0];
        *candidates = coordinates(program, machine, entry, observation, frames)?
            .into_iter()
            .filter(|coordinate| coordinate.primitive == primitive)
            .collect();
        if candidates.is_empty() {
            return Some(Err(
                "a private member has no compatible natural rank coordinate",
            ));
        }
    }
    let mut edges = Vec::new();
    for (source, machine) in machines.iter().enumerate() {
        let entry = &program.machine_states(machine)[0];
        for (statement, _) in program
            .statement_table
            .indexed_statements(entry.statement_nodes)
        {
            for (ordinal, call) in
                validation::collect_call_sites_in_statement(program, machine, entry, statement)
                    .into_iter()
                    .enumerate()
            {
                let target = call.target_symbol(program);
                let Some(destination) = machines.iter().position(|machine| {
                    target == machine.symbol || target == program.machine_states(machine)[0].symbol
                }) else {
                    continue;
                };
                let Some(arrival) =
                    validation::transition_call_target(program, machine, entry, statement, ordinal)
                else {
                    return Some(Err(
                        "a recurrent natural call has no exact terminal arrival",
                    ));
                };
                if !call.completes_transition(program, arrival) {
                    return Some(Err(
                        "a recurrent natural call is not the whole terminal result",
                    ));
                }
                let arguments = validation::call_site_argument_expressions(program, &call);
                if arguments.iter().any(|argument| {
                    !frames.is_some_and(|frames| {
                        frames
                            .expression_write_frame(machine, *argument)
                            .into_complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                    })
                }) {
                    return Some(Err(
                        "natural rank operands have an unpreserved evaluation prefix",
                    ));
                }
                let Some(values) =
                    CurrentScalarValues::at(program, machine, entry, statement, arrival, frames)
                else {
                    return Some(Err(
                        "a natural call has no complete current-value observation",
                    ));
                };
                edges.push(Edge {
                    source,
                    destination,
                    statement,
                    arguments,
                    values,
                });
            }
        }
    }
    let mut chosen = Vec::new();
    let mut result = None;
    search(
        program,
        &machines,
        &candidates,
        &edges,
        &mut chosen,
        &mut result,
    );
    Some(
        result.ok_or("no exact private-coordinate assignment proves every edge and complete cycle"),
    )
}

impl Coordinate {
    fn current(
        &self,
        values: &CurrentScalarValues<'_>,
        actual: ExpressionHandle,
    ) -> Option<ScopedArithmeticValue> {
        match self.observation {
            validation::RankPremiseObservation::Value => {
                values.projected_scalar_value(actual, &self.path)
            }
            validation::RankPremiseObservation::Length => {
                values.projected_collection_length(actual, &self.path)
            }
        }
    }
}

/// Capture eligible coordinates once before the first admitted executable
/// statement. An invalid target applies no branch guard or operand effects.
/// Each candidate owns only its selected value, not a second field roster.
fn coordinates(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    observation: validation::RankPremiseObservation,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<Coordinate>> {
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let first = *order.indices().first()?;
    let values =
        CurrentScalarValues::at(program, machine, state, first, Default::default(), frames)?;
    let mut coordinates = Vec::new();
    for (position, parameter) in program.state_parameters(state).iter().enumerate() {
        if parameter.relevance.is_erased() || parameter.is_const {
            continue;
        }
        if let Some(primitive) = observation_carrier(program, state, parameter.symbol, observation)
        {
            let entry_value = match observation {
                validation::RankPremiseObservation::Value => {
                    values.initial_symbol(parameter.symbol)
                }
                validation::RankPremiseObservation::Length => {
                    values.initial_collection_length(parameter.symbol)
                }
            };
            if let Some(entry_value) = entry_value {
                coordinates.push(Coordinate {
                    parameter: parameter.symbol,
                    path: Vec::new(),
                    entry_value,
                    position,
                    primitive,
                    observation,
                });
            }
        }
        // Entry seeds retain ordinary fields under their admitted root access.
        // The runtime projection owner independently authenticates each selected
        // scalar path, including erasure and exact declaration identities.
        if observation == validation::RankPremiseObservation::Value {
            for (path, primitive, value) in values.scalar_field_observations(parameter.symbol) {
                let path = path
                    .iter()
                    .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                    .collect::<Vec<_>>();
                let Some(field) = validation::structural_scalar_field_at_coordinate(
                    program,
                    program.state_parameters(state),
                    parameter.symbol,
                    &path,
                ) else {
                    continue;
                };
                if field.primitive_type != primitive {
                    continue;
                }
                coordinates.push(Coordinate {
                    parameter: parameter.symbol,
                    path,
                    entry_value: value.clone(),
                    position,
                    primitive,
                    observation,
                });
            }
        } else {
            for (path, value) in values.length_field_observations(parameter.symbol) {
                let path = path
                    .iter()
                    .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                    .collect::<Vec<_>>();
                // Extents also include fixed arrays. The ordinary coordinate
                // owner authenticates that this selected leaf is a descriptor
                // observation before it can enter a Slice::Length component.
                if validation::ranking_slice_length_observation(
                    program,
                    state,
                    &validation::RankSubjectCoordinate {
                        parameter: parameter.symbol,
                        path: path.clone(),
                    },
                )
                .is_none()
                {
                    continue;
                }
                coordinates.push(Coordinate {
                    parameter: parameter.symbol,
                    path,
                    entry_value: value.clone(),
                    position,
                    primitive: PrimitiveType::U64,
                    observation,
                });
            }
        }
    }
    Some(coordinates)
}

fn observation_carrier(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    parameter: SymbolHandle,
    observation: validation::RankPremiseObservation,
) -> Option<PrimitiveType> {
    match observation {
        validation::RankPremiseObservation::Value => {
            let parameter = program.state_parameter_by_symbol(state, parameter)?;
            let primitive = program.primitive_type_reference(parameter.type_reference)?;
            primitive.is_fixed_integer().then_some(primitive)
        }
        validation::RankPremiseObservation::Length => {
            validation::ranking_slice_length_observation(
                program,
                state,
                &validation::RankSubjectCoordinate {
                    parameter,
                    path: Vec::new(),
                },
            )?;
            Some(PrimitiveType::U64)
        }
    }
}

fn compare(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    edge: &Edge<'_>,
    source: &Coordinate,
    destination: &Coordinate,
) -> Option<bool> {
    let actual = *edge.arguments.get(destination.position)?;
    let initial = &source.entry_value;
    let next = destination.current(&edge.values, actual)?;
    let zero = ScopedArithmeticValue::Integer(BigInt::zero());
    let proves = |operator, left, right| {
        edge.values
            .proves_values_relation(program, machine, operator, left, right)
    };
    (proves(BinaryOperator::GreaterOrEqual, initial, &zero)
        && proves(BinaryOperator::GreaterOrEqual, &next, &zero)
        && proves(BinaryOperator::LessOrEqual, &next, initial))
    .then(|| proves(BinaryOperator::Less, &next, initial))
}

fn search(
    program: &dyn validation::CheckingInput,
    machines: &[&typed_trees::machine::Machine],
    candidates: &[Vec<Coordinate>],
    edges: &[Edge<'_>],
    chosen: &mut Vec<usize>,
    result: &mut Option<ValidatedRuntimeNaturalComponent>,
) {
    if chosen.len() < candidates.len() {
        for selection in 0..candidates[chosen.len()].len() {
            chosen.push(selection);
            if edges.iter().all(|edge| {
                if edge.source >= chosen.len() || edge.destination >= chosen.len() {
                    return true;
                }
                compare(
                    program,
                    machines[edge.source],
                    edge,
                    &candidates[edge.source][chosen[edge.source]],
                    &candidates[edge.destination][chosen[edge.destination]],
                )
                .is_some()
            }) {
                search(program, machines, candidates, edges, chosen, result);
            }
            chosen.pop();
            if result.is_some() {
                return;
            }
        }
        return;
    }
    let mut weak = vec![Vec::new(); machines.len()];
    let mut retained = Vec::new();
    for edge in edges {
        let Some(strict) = compare(
            program,
            machines[edge.source],
            edge,
            &candidates[edge.source][chosen[edge.source]],
            &candidates[edge.destination][chosen[edge.destination]],
        ) else {
            return;
        };
        if !strict {
            weak[edge.source].push(edge.destination);
        }
        let Ok(statement_ordinal) = u32::try_from(edge.statement) else {
            return;
        };
        retained.push(validation::ValidatedRuntimeNaturalEdge {
            source: program.machine_states(machines[edge.source])[0].symbol,
            target: program.machine_states(machines[edge.destination])[0].symbol,
            statement_ordinal,
            strict,
        });
    }
    if super::graph::strongly_connected_components(&weak)
        .iter()
        .any(|component| super::graph::component_is_cyclic(&weak, component))
    {
        return;
    }
    let states = machines
        .iter()
        .enumerate()
        .map(|(position, machine)| {
            let coordinate = &candidates[position][chosen[position]];
            Some(validation::ValidatedRuntimeNaturalState {
                state: program.machine_states(machine)[0].symbol,
                parameter: coordinate.parameter,
                coordinate: validation::RankSubjectCoordinate {
                    parameter: coordinate.parameter,
                    path: coordinate.path.clone(),
                },
                parameter_position: u32::try_from(coordinate.position).ok()?,
                primitive_type: coordinate.primitive,
                observation: coordinate.observation,
            })
        })
        .collect::<Option<Vec<_>>>();
    if let Some(states) = states {
        *result = Some(ValidatedRuntimeNaturalComponent {
            states,
            edges: retained,
        });
    }
}
