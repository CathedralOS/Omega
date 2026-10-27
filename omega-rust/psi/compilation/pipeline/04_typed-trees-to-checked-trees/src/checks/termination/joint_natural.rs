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

struct Coordinate {
    parameter: SymbolHandle,
    position: usize,
    primitive: PrimitiveType,
}

struct Edge<'program> {
    source: usize,
    destination: usize,
    statement: usize,
    arguments: Vec<ExpressionHandle>,
    values: CurrentScalarValues<'program>,
}

/// Ranged and non-natural views retain their existing measure/endpoint owner.
/// An unranged natural coordinate is checked directly at every call, including
/// when a private participant has no authored ranking clause of its own.
pub(super) fn select(
    program: &dyn validation::CheckingInput,
    component: &[usize],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Result<ValidatedRuntimeNaturalComponent, &'static str>> {
    let machines = component
        .iter()
        .map(|position| &program.machines()[*position])
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    let mut primitive = None;
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
        if witness.rank_range.is_some() {
            return None;
        }
        let subjects = typed_trees::ranking::resolve_machine_witness_subjects(program, machine)?;
        let arguments =
            typed_trees::ranking::resolve_machine_witness_view_arguments(program, machine)?;
        if !matches!(
            RankingOrder::resolve_witness(program, entry, witness, &subjects, &arguments),
            OrderResolution::Resolved(RankingOrder::NatDescending)
        ) {
            return None;
        }
        let [subject] = subjects.as_slice() else {
            return None;
        };
        let place = validation::canonical_place_from_expression_in_state(
            program,
            entry.symbol,
            0,
            *subject,
        )?;
        if !place.segments.is_empty() {
            return None;
        }
        let parameter = place.root.as_symbol()?;
        let (position, declaration) = program
            .state_parameters(entry)
            .iter()
            .enumerate()
            .find(|(_, declaration)| declaration.symbol == parameter)?;
        let carrier = program.primitive_type_reference(declaration.type_reference)?;
        if !carrier.is_fixed_integer() || primitive.is_some_and(|previous| previous != carrier) {
            return None;
        }
        primitive = Some(carrier);
        candidates.push(vec![Coordinate {
            parameter,
            position,
            primitive: carrier,
        }]);
    }
    let primitive = primitive?;
    for (machine, candidates) in machines.iter().zip(&mut candidates) {
        if !candidates.is_empty() {
            continue;
        }
        // Public promises require authored evidence. Private coordinate
        // inference changes neither a public schema nor a source signature.
        if machine.is_public
            || machine
                .termination_plan
                .interface
                .published()
                .is_some_and(language_semantics::TerminationGuarantee::promises_termination)
        {
            return None;
        }
        let entry = &program.machine_states(machine)[0];
        for (position, parameter) in program.state_parameters(entry).iter().enumerate() {
            if !parameter.relevance.is_erased()
                && !parameter.is_const
                && program.primitive_type_reference(parameter.type_reference) == Some(primitive)
            {
                candidates.push(Coordinate {
                    parameter: parameter.symbol,
                    position,
                    primitive,
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
                    arguments: arguments.to_vec(),
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

fn compare(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    edge: &Edge<'_>,
    source: &Coordinate,
    destination: &Coordinate,
) -> Option<bool> {
    let initial = edge.values.initial_symbol(source.parameter)?;
    let next = edge
        .values
        .current(*edge.arguments.get(destination.position)?)
        .map(ScopedArithmeticValue::Term)?;
    let zero = ScopedArithmeticValue::Integer(BigInt::zero());
    let proves = |operator, left, right| {
        edge.values
            .proves_values_relation(program, machine, operator, left, right)
    };
    (proves(BinaryOperator::GreaterOrEqual, &initial, &zero)
        && proves(BinaryOperator::GreaterOrEqual, &next, &zero)
        && proves(BinaryOperator::LessOrEqual, &next, &initial))
    .then(|| proves(BinaryOperator::Less, &next, &initial))
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
                    path: Vec::new(),
                },
                parameter_position: u32::try_from(coordinate.position).ok()?,
                primitive_type: coordinate.primitive,
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
