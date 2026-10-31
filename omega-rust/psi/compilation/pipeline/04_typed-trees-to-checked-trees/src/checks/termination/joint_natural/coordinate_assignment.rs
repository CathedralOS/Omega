//! Choose complete participant assignments and prove the weak-edge DAG.

use super::{
    coordinates::Rank,
    edge_observations::{Edge, compare},
    views::Order,
};
use validation::ValidatedRuntimeNaturalComponent;

pub(super) fn select(
    program: &dyn validation::CheckingInput,
    machines: &[&typed_trees::machine::Machine],
    candidates: &[Vec<Rank>],
    edges: &[Edge<'_>],
    order: Order,
) -> Option<ValidatedRuntimeNaturalComponent> {
    let mut result = None;
    search(
        program,
        machines,
        candidates,
        edges,
        order,
        &mut Vec::new(),
        &mut result,
    );
    result
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
    if super::super::graph::strongly_connected_components(&weak)
        .iter()
        .any(|component| super::super::graph::component_is_cyclic(&weak, component))
    {
        return;
    }
    let states = machines
        .iter()
        .enumerate()
        .map(|(position, machine)| {
            let coordinate = &candidates[position][chosen[position]].coordinate;
            Some(validation::ValidatedRuntimeNaturalState {
                measure: candidates[position][chosen[position]]
                    .application
                    .map_or(symbols::SymbolHandle::invalid(), |application| {
                        application.measure
                    }),
                state: program.machine_states(machine)[0].symbol,
                parameter: coordinate.parameter,
                coordinate: validation::RankSubjectCoordinate {
                    parameter: coordinate.parameter,
                    path: coordinate.path.clone(),
                },
                entry_indexes: coordinate.entry_indexes.clone(),
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
