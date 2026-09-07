//! Exact entry permutations seed named self-loop range invariants. Cross-state
//! cycles and computed entry transports need a separate arrival judgment.

use super::{graph, patterns, preserved_entry_prefix};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;

pub(super) fn prove<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    range: ExpressionHandle,
    measure: validation::RankingRangeMeasure,
    frames: Option<&validation::CallFrameResolver<'program>>,
) -> bool {
    let states = program.machine_states(machine);
    let Some(root) = states.first() else {
        return false;
    };
    // Every non-entry state is an independently seeded self-loop. No incoming
    // edge may evade the arrival proof by passing through another named state.
    if graph::machine_adjacency(program, machine)
        .iter()
        .enumerate()
        .any(|(source, targets)| source != 0 && targets.iter().any(|target| *target != source))
    {
        return false;
    }
    for state in states.iter().skip(1) {
        let arrivals = patterns::edges_to_state(program, root, state.symbol);
        let Some(first) = arrivals.first() else {
            return false;
        };
        let Some(entry_parameters) = arrival_parameters(program, root, state, first.arguments)
        else {
            return false;
        };
        for arrival in arrivals {
            if arrival_parameters(program, root, state, arrival.arguments).as_ref()
                != Some(&entry_parameters)
            {
                return false;
            }
            let Some(prefix) =
                preserved_entry_prefix(program, machine, root, frames, arrival.statement_ordinal)
            else {
                return false;
            };
            // The checked permutation puts actuals back into the entry
            // telescope; the query then checks membership and endpoint pinning
            // under the exact arrival's live prefix and guards, not its name.
            let Some(arguments) = program
                .state_parameters(root)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| {
                    let position = entry_parameters
                        .iter()
                        .position(|symbol| *symbol == parameter.symbol)?;
                    arrival.arguments.get(position).copied()
                })
                .collect::<Option<Vec<_>>>()
            else {
                return false;
            };
            let guards = arrival
                .guards
                .iter()
                .map(|guard| (guard.expression, guard.holds))
                .collect::<Vec<_>>();
            if !validation::prove_ranking_range_edge(
                program, machine, root, range, measure, &guards, &prefix, &arguments,
            )
            .is_some_and(|proof| proof.membership_and_pinning)
            {
                return false;
            }
        }
        let edges = patterns::edges_to_state(program, state, state.symbol);
        if edges.is_empty() {
            return false;
        }
        for edge in edges {
            let Some(prefix) =
                preserved_entry_prefix(program, machine, state, frames, edge.statement_ordinal)
            else {
                return false;
            };
            let guards = edge
                .guards
                .iter()
                .map(|guard| (guard.expression, guard.holds))
                .collect::<Vec<_>>();
            if !validation::prove_ranking_range_named_state_edge(
                program,
                machine,
                state,
                range,
                measure,
                &entry_parameters,
                &guards,
                &prefix,
                edge.arguments,
            )
            .is_some_and(|proof| proof.membership_and_pinning && proof.strictly_decreases)
            {
                return false;
            }
        }
    }
    true
}

/// Destination ordinal -> exact entry symbol. Only a complete one-to-one
/// scalar permutation can seed this tier; spelling never selects a parameter.
fn arrival_parameters(
    program: &TypedTrees,
    root: &State,
    target: &State,
    arguments: &[ExpressionHandle],
) -> Option<Vec<SymbolHandle>> {
    let source_parameters = program.state_parameters(root);
    let target_parameters = program.state_parameters(target);
    if arguments.len()
        != source_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count()
        || arguments.len()
            != target_parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count()
    {
        return None;
    }
    let mut parameters = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let ExpressionNode::Name(name) = program.expression_table.expression(*argument) else {
            return None;
        };
        if !name.symbol.is_valid()
            || name.head_symbol != name.symbol
            || parameters.contains(&name.symbol)
            || !source_parameters.iter().any(|parameter| {
                !parameter.is_self
                    && !parameter.is_mutable
                    && !parameter.is_const
                    && parameter.symbol == name.symbol
            })
        {
            return None;
        }
        parameters.push(name.symbol);
    }
    Some(parameters)
}
