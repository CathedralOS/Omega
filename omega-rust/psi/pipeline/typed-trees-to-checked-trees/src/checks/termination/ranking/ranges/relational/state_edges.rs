//! Entry-rooted identity transfers establish exact state telescopes. Every
//! arrival then rechecks range membership; cyclic edges also owe strict descent.

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
    let mut adjacency = graph::machine_adjacency(program, machine);
    // Topology needs one pair per destination; the occurrence reader below
    // still checks every distinct guarded transition to that destination.
    for targets in &mut adjacency {
        targets.sort_unstable();
        targets.dedup();
    }
    // Re-entering the root would require re-establishing its arbitrary machine
    // requirements. Range membership alone is not that judgment.
    if adjacency.iter().skip(1).any(|targets| targets.contains(&0)) {
        return false;
    }
    let Some(mappings) = discover_mappings(program, machine, &adjacency) else {
        return false;
    };
    let components = graph::strongly_connected_components(&adjacency);
    for (source_position, source) in states.iter().enumerate() {
        for &target_position in &adjacency[source_position] {
            if target_position == 0 {
                continue;
            }
            let target = &states[target_position];
            let cyclic = components.iter().any(|component| {
                component.contains(&source_position)
                    && component.contains(&target_position)
                    && graph::component_is_cyclic(&adjacency, component)
            });
            for edge in patterns::edges_to_state(program, source, target.symbol) {
                let Some(prefix) = preserved_entry_prefix(
                    program,
                    machine,
                    source,
                    frames,
                    edge.statement_ordinal,
                ) else {
                    return false;
                };
                let guards = edge
                    .guards
                    .iter()
                    .map(|guard| (guard.expression, guard.holds))
                    .collect::<Vec<_>>();
                if !validation::prove_ranking_range_transition(
                    program,
                    machine,
                    range,
                    measure,
                    validation::RankingRangeState {
                        state: source,
                        entry_parameters: &mappings[source_position],
                    },
                    validation::RankingRangeState {
                        state: target,
                        entry_parameters: &mappings[target_position],
                    },
                    &guards,
                    &prefix,
                    edge.arguments,
                )
                .is_some_and(|proof| {
                    proof.membership_and_pinning && (!cyclic || proof.strictly_decreases)
                }) {
                    return false;
                }
            }
        }
    }
    true
}

/// Only identity edges choose a telescope. Arithmetic edges may use a target
/// already discovered elsewhere, but cannot invent which subject it ranks.
/// Each state receives one finite correspondence and enters the worklist once.
/// Conflicting identity arrivals reject even after a target was processed.
fn discover_mappings(
    program: &TypedTrees,
    machine: &Machine,
    adjacency: &[Vec<usize>],
) -> Option<Vec<Vec<SymbolHandle>>> {
    let states = program.machine_states(machine);
    let root = states.first()?;
    let mut mappings = vec![None; states.len()];
    mappings[0] = Some(
        program
            .state_parameters(root)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>(),
    );
    let mut pending_states = vec![0];
    while let Some(source_position) = pending_states.pop() {
        let source = states.get(source_position)?;
        let source_mapping = mappings[source_position].as_ref()?.clone();
        for &target_position in &adjacency[source_position] {
            if target_position == 0 {
                continue;
            }
            let target = states.get(target_position)?;
            for edge in patterns::edges_to_state(program, source, target.symbol) {
                if !edge.arguments.iter().all(|argument| {
                    matches!(
                        program.expression_table.expression(*argument),
                        ExpressionNode::Name(_)
                    )
                }) {
                    continue;
                }
                let incoming =
                    identity_mapping(program, source, target, &source_mapping, edge.arguments)?;
                match &mappings[target_position] {
                    Some(existing) if *existing != incoming => return None,
                    Some(_) => {}
                    None => {
                        mappings[target_position] = Some(incoming);
                        pending_states.push(target_position);
                    }
                }
            }
        }
    }
    mappings.into_iter().collect()
}

/// Compose destination formal ordinal -> exact source parameter -> entry
/// subject. States may drop or repeat unrelated parameters. The arithmetic
/// query independently requires an unambiguous slot for every rank input.
fn identity_mapping(
    program: &TypedTrees,
    source: &State,
    target: &State,
    source_mapping: &[SymbolHandle],
    arguments: &[ExpressionHandle],
) -> Option<Vec<SymbolHandle>> {
    let source_parameters = program
        .state_parameters(source)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if source_mapping.len() != source_parameters.len()
        || arguments.len()
            != program
                .state_parameters(target)
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
        if !name.symbol.is_valid() || name.head_symbol != name.symbol {
            return None;
        }
        let source_position = source_parameters.iter().position(|parameter| {
            parameter.symbol == name.symbol && !parameter.is_mutable && !parameter.is_const
        })?;
        let entry_symbol = source_mapping[source_position];
        parameters.push(entry_symbol);
    }
    Some(parameters)
}
