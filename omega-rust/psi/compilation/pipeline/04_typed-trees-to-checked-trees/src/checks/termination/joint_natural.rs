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

#[derive(Clone)]
struct Coordinate {
    parameter: SymbolHandle,
    path: Vec<facts::PlaceSegment>,
    entry_value: ScopedArithmeticValue,
    position: usize,
    primitive: PrimitiveType,
    observation: validation::RankPremiseObservation,
}

#[derive(Clone)]
enum Bound {
    Authored {
        expression: ExpressionHandle,
        entry_value: ScopedArithmeticValue,
    },
    Coordinate(Coordinate),
    Closed(ScopedArithmeticValue),
}

#[derive(Clone)]
struct Rank {
    coordinate: Coordinate,
    bound: Option<Bound>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Order {
    Descending,
    BoundedDistance,
    IncreasingTo,
}

impl Order {
    fn function(self) -> checked_trees::CheckedNaturalRankFunction {
        match self {
            Self::Descending => checked_trees::CheckedNaturalRankFunction::Identity,
            Self::BoundedDistance | Self::IncreasingTo => {
                checked_trees::CheckedNaturalRankFunction::Complement
            }
        }
    }
}

struct Edge<'program> {
    source: usize,
    destination: usize,
    statement: usize,
    arguments: &'program [ExpressionHandle],
    site: validation::CallSite<'program>,
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
    let mut selected_order = None;
    let mut available = Vec::new();
    let mut entry_values = Vec::new();
    for machine in &machines {
        // Source declarations have one entry. Legacy multi-state arrival
        // telescopes remain with the ordinary full-coordinate owner.
        let [entry] = program.machine_states(machine) else {
            return None;
        };
        let execution = program
            .statement_table
            .admitted_execution_order(entry.statement_nodes);
        let first = *execution.indices().first()?;
        let values =
            CurrentScalarValues::at(program, machine, entry, first, Default::default(), frames)?;
        available.push(coordinates(program, entry, &values));
        entry_values.push(values);
        let Some(witness) = machine.termination_plan.implementation_witness.as_ref() else {
            candidates.push(Vec::new());
            continue;
        };
        let subjects = typed_trees::ranking::resolve_machine_witness_subjects(program, machine)?;
        let arguments =
            typed_trees::ranking::resolve_machine_witness_view_arguments(program, machine)?;
        let (order, observation, subject, upper, measure) =
            match RankingOrder::resolve_witness(program, entry, witness, &subjects, &arguments) {
                OrderResolution::Resolved(RankingOrder::NatDescending) if subjects.len() == 1 => (
                    Order::Descending,
                    validation::RankPremiseObservation::Value,
                    subjects[0],
                    None,
                    validation::RankingRangeMeasure::Single(subjects[0]),
                ),
                OrderResolution::Resolved(RankingOrder::SliceLength) if subjects.len() == 1 => (
                    Order::Descending,
                    validation::RankPremiseObservation::Length,
                    subjects[0],
                    None,
                    validation::RankingRangeMeasure::SliceLength(subjects[0]),
                ),
                OrderResolution::Resolved(RankingOrder::BoundedDistance) if subjects.len() == 2 => {
                    (
                        Order::BoundedDistance,
                        validation::RankPremiseObservation::Value,
                        subjects[0],
                        Some(subjects[1]),
                        validation::RankingRangeMeasure::Distance {
                            lower: subjects[0],
                            upper: subjects[1],
                        },
                    )
                }
                OrderResolution::Resolved(RankingOrder::IncreasingTo(upper))
                    if subjects.len() == 1 =>
                {
                    (
                        Order::IncreasingTo,
                        validation::RankPremiseObservation::Value,
                        subjects[0],
                        Some(upper),
                        validation::RankingRangeMeasure::IncreasingTo {
                            subject: subjects[0],
                            limit: upper,
                        },
                    )
                }
                _ => return None,
            };
        let place = validation::canonical_place_from_expression_in_state(
            program,
            entry.symbol,
            0,
            subject,
        )?;
        let parameter = place.root.as_symbol()?;
        let selected = available
            .last()?
            .iter()
            .find(|coordinate| {
                coordinate.parameter == parameter
                    && coordinate.path == place.segments
                    && coordinate.observation == observation
            })?
            .clone();
        let carrier = selected.primitive;
        if selected_order.is_some_and(|previous| previous != (order, carrier)) {
            return None;
        }
        if order != Order::Descending && order.function().complement_ceiling(carrier).is_none() {
            return None;
        }
        selected_order = Some((order, carrier));
        if let Err(reason) = ranges::entry(program, machine, measure) {
            return Some(Err(reason));
        }
        let values = entry_values.last()?;
        let bound = if let Some(expression) = upper {
            let value = values.current_value(expression)?;
            if order == Order::BoundedDistance
                && !values.proves_values_relation(
                    program,
                    machine,
                    BinaryOperator::LessOrEqual,
                    &selected.entry_value,
                    &value,
                )
            {
                return Some(Err(
                    "a bounded cursor rank lacks independent entry formation",
                ));
            }
            Some(Bound::Authored {
                expression,
                entry_value: value,
            })
        } else {
            None
        };
        candidates.push(vec![Rank {
            coordinate: selected,
            bound,
        }]);
    }
    let (order, primitive) = selected_order?;
    let closed_bounds = candidates
        .iter()
        .flatten()
        .filter_map(|rank| rank.bound.as_ref())
        .filter_map(|bound| match bound.entry() {
            value @ ScopedArithmeticValue::Integer(_) => Some(value.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for (position, (machine, candidates)) in machines.iter().zip(&mut candidates).enumerate() {
        if !candidates.is_empty() {
            continue;
        }
        // Private correspondence changes neither public schemas nor source
        // signatures. Only exact runtime leaves in this invocation participate.
        if machine.is_public {
            return None;
        }
        for coordinate in available[position]
            .iter()
            .filter(|coordinate| coordinate.primitive == primitive)
        {
            if order == Order::Descending {
                candidates.push(Rank {
                    coordinate: coordinate.clone(),
                    bound: None,
                });
                continue;
            }
            let bounds = available[position]
                .iter()
                .cloned()
                .map(Bound::Coordinate)
                .chain(closed_bounds.iter().cloned().map(Bound::Closed));
            for bound in bounds {
                if order == Order::BoundedDistance
                    && !entry_values[position].proves_values_relation(
                        program,
                        machine,
                        BinaryOperator::LessOrEqual,
                        &coordinate.entry_value,
                        bound.entry(),
                    )
                {
                    continue;
                }
                candidates.push(Rank {
                    coordinate: coordinate.clone(),
                    bound: Some(bound),
                });
            }
        }
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
                    site: call,
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
        order,
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

impl Bound {
    fn entry(&self) -> &ScopedArithmeticValue {
        match self {
            Self::Authored { entry_value, .. } => entry_value,
            Self::Coordinate(coordinate) => &coordinate.entry_value,
            Self::Closed(value) => value,
        }
    }

    fn arriving(
        &self,
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        edge: &Edge<'_>,
    ) -> Option<ScopedArithmeticValue> {
        match self {
            Self::Authored { expression, .. } => {
                let state = &program.machine_states(machine)[0];
                Some(ScopedArithmeticValue::Term(
                    edge.values.bind_call_parameters(
                        machine,
                        state,
                        *expression,
                        program.state_parameters(state),
                        edge.arguments,
                        edge.site,
                    )?,
                ))
            }
            Self::Coordinate(coordinate) => {
                coordinate.current(&edge.values, *edge.arguments.get(coordinate.position)?)
            }
            Self::Closed(value) => Some(value.clone()),
        }
    }
}

/// Capture eligible coordinates once before the first admitted executable
/// statement. An invalid target applies no branch guard or operand effects.
/// Each candidate owns only its selected value, not a second field roster.
fn coordinates(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    values: &CurrentScalarValues<'_>,
) -> Vec<Coordinate> {
    let mut coordinates = Vec::new();
    for observation in [
        validation::RankPremiseObservation::Value,
        validation::RankPremiseObservation::Length,
    ] {
        for (position, parameter) in program.state_parameters(state).iter().enumerate() {
            if parameter.relevance.is_erased() || parameter.is_const {
                continue;
            }
            if let Some(primitive) =
                observation_carrier(program, state, parameter.symbol, observation)
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
            // The entry seed walks owned inline records only. Borrowed storage
            // and erased/static parameters cannot acquire a private read here.
            if observation == validation::RankPremiseObservation::Value {
                for (path, primitive, value) in values.scalar_field_observations(parameter.symbol) {
                    coordinates.push(Coordinate {
                        parameter: parameter.symbol,
                        path: path
                            .iter()
                            .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                            .collect(),
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
    }
    coordinates
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
    machines: &[&typed_trees::machine::Machine],
    edge: &Edge<'_>,
    source: &Rank,
    destination: &Rank,
    order: Order,
) -> Option<bool> {
    let actual = *edge.arguments.get(destination.coordinate.position)?;
    let initial = &source.coordinate.entry_value;
    let next = destination.coordinate.current(&edge.values, actual)?;
    let zero = ScopedArithmeticValue::Integer(BigInt::zero());
    let proves = |operator, left: &ScopedArithmeticValue, right: &ScopedArithmeticValue| {
        edge.values
            .proves_values_relation(program, machines[edge.source], operator, left, right)
    };
    let strict_allowed = if order == Order::Descending {
        true
    } else {
        let bound = source.bound.as_ref()?.entry();
        let arriving =
            destination
                .bound
                .as_ref()?
                .arriving(program, machines[edge.destination], edge)?;
        // MAX - cursor agrees with Distance ordering under a conserved upper.
        // For IncreasingTo it conservatively refines weak clamped edges; only
        // a source below the limit can contribute strict descent. A private member
        // must supply that coordinate from its own invocation, not the root's
        // ambient scope. Distance formation is independently owed at both ends.
        if !proves(BinaryOperator::Equal, bound, &arriving) {
            return None;
        }
        if order == Order::BoundedDistance
            && !(proves(BinaryOperator::LessOrEqual, initial, bound)
                && proves(BinaryOperator::LessOrEqual, &next, &arriving))
        {
            return None;
        }
        order != Order::IncreasingTo || proves(BinaryOperator::Less, initial, bound)
    };
    let normalize = |value: &ScopedArithmeticValue| {
        if order == Order::Descending {
            Some(value.clone())
        } else {
            let (minimum, maximum) = source.coordinate.primitive.integer_bounds()?;
            // Both exact coordinates have this fixed-width carrier, and call
            // admission separately checks the arriving actual. Captured atoms
            // retain identity and signedness, but do not carry their physical
            // ceiling. Supply that declared carrier evidence before forming
            // the total complement; an authored rank interval supplies none.
            let value = ScopedArithmeticValue::BoundedInteger {
                value: Box::new(value.clone()),
                minimum: Some(BigInt::from_i128(minimum)),
                maximum: Some(BigInt::from_i128(maximum)),
            };
            Some(ScopedArithmeticValue::IntegerOperation {
                operator: BinaryOperator::Subtract,
                left: Box::new(ScopedArithmeticValue::Integer(BigInt::from_u64(
                    order
                        .function()
                        .complement_ceiling(source.coordinate.primitive)?,
                ))),
                right: Box::new(value),
            })
        }
    };
    let initial = normalize(initial)?;
    let next = normalize(&next)?;
    (proves(BinaryOperator::GreaterOrEqual, &initial, &zero)
        && proves(BinaryOperator::GreaterOrEqual, &next, &zero)
        && proves(BinaryOperator::LessOrEqual, &next, &initial))
    .then(|| strict_allowed && proves(BinaryOperator::Less, &next, &initial))
}

fn search(
    program: &dyn validation::CheckingInput,
    machines: &[&typed_trees::machine::Machine],
    candidates: &[Vec<Rank>],
    edges: &[Edge<'_>],
    order: Order,
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
                    machines,
                    edge,
                    &candidates[edge.source][chosen[edge.source]],
                    &candidates[edge.destination][chosen[edge.destination]],
                    order,
                )
                .is_some()
            }) {
                search(program, machines, candidates, edges, order, chosen, result);
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
            machines,
            edge,
            &candidates[edge.source][chosen[edge.source]],
            &candidates[edge.destination][chosen[edge.destination]],
            order,
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
            let coordinate = &candidates[position][chosen[position]].coordinate;
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
                function: order.function(),
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
