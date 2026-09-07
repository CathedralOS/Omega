//! Entry-rooted parameter transfers establish exact state telescopes. Every
//! arrival then rechecks range membership; cyclic edges also owe strict descent.

use super::{graph, patterns, preserved_entry_prefix};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;

pub(super) fn prove<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    range: ExpressionHandle,
    measure: validation::RankingRangeMeasure,
    frames: Option<&validation::CallFrameResolver<'program>>,
    premises: validation::RankingRangePremises,
) -> bool {
    let states = program.machine_states(machine);
    let mut adjacency = graph::machine_adjacency(program, machine);
    // Topology needs one pair per destination; the occurrence reader below
    // still checks every distinct guarded transition to that destination.
    for targets in &mut adjacency {
        targets.sort_unstable();
        targets.dedup();
    }
    let entry_is_initial = !adjacency.iter().any(|targets| targets.contains(&0));
    let Some(mappings) = discover_mappings(program, machine, &adjacency) else {
        return false;
    };
    let components = graph::strongly_connected_components(&adjacency);
    for (source_position, source) in states.iter().enumerate() {
        for &target_position in &adjacency[source_position] {
            if source_position == 0 && target_position == 0 {
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
                    if source_position == 0
                        && entry_is_initial
                        && matches!(premises, validation::RankingRangePremises::RankInvariant)
                    {
                        validation::RankingRangePremises::InitialEntry
                    } else {
                        premises
                    },
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

/// Identity transfers anchor the first closure. Single-parameter computations
/// can then establish remaining telescopes without choosing a different subject.
/// Each state enters the worklist once per tier. Every eligible incoming edge
/// checks its proposal, including edges to already-processed destinations, so
/// provisional discovery order cannot resolve conflicting correspondences.
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
    for computed in [false, true] {
        let anchored = mappings.iter().map(Option::is_some).collect::<Vec<_>>();
        let mut pending_states = anchored
            .iter()
            .enumerate()
            .filter_map(|(position, known)| known.then_some(position))
            .collect::<Vec<_>>();
        while let Some(source_position) = pending_states.pop() {
            let source = states.get(source_position)?;
            let source_mapping = mappings[source_position].as_ref()?.clone();
            for &target_position in &adjacency[source_position] {
                if target_position == 0 {
                    continue;
                }
                let target = states.get(target_position)?;
                for edge in patterns::edges_to_state(program, source, target.symbol) {
                    let identity = edge.arguments.iter().all(|argument| {
                        matches!(
                            program.expression_table.expression(*argument),
                            ExpressionNode::Name(_)
                        )
                    });
                    if !identity && (!computed || anchored[target_position]) {
                        // Arithmetic actuals use an identity-anchored target;
                        // they do not redefine its parameter correspondence.
                        continue;
                    }
                    let Some(incoming) =
                        argument_mapping(program, source, target, &source_mapping, edge.arguments)
                    else {
                        if identity {
                            return None;
                        }
                        continue;
                    };
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
    }
    mappings.into_iter().collect()
}

/// Compose destination formal ordinal -> exact source parameter -> entry
/// subject. States may drop or repeat unrelated parameters. The arithmetic
/// query independently requires an unambiguous slot for every rank input.
fn argument_mapping(
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
        let subject = argument_subject(program, *argument, 0)?;
        let source_position = source_parameters.iter().position(|parameter| {
            subject.is_valid()
                && parameter.symbol == subject
                && !parameter.is_mutable
                && !parameter.is_const
        })?;
        let entry_symbol = source_mapping[source_position];
        parameters.push(entry_symbol);
    }
    Some(parameters)
}

/// Discover a dependency, not a value equality or an arithmetic theorem. The
/// ordinary edge query still checks selected builtin meaning and every rank
/// obligation before this provisional correspondence can authorize anything.
/// Count distinct current symbols before translating them to entry subjects:
/// two copies with shared ancestry may have diverged. Zero denotes a literal
/// subtree, which cannot establish a subject on its own.
fn argument_subject(
    program: &TypedTrees,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<SymbolHandle> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => Some(SymbolHandle::default()),
        ExpressionNode::Name(name) if name.symbol.is_valid() && name.head_symbol == name.symbol => {
            Some(name.symbol)
        }
        ExpressionNode::Atomic(atomic) => argument_subject(program, atomic.value, depth + 1),
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Modulo
            ) =>
        {
            let left = argument_subject(program, binary.left, depth + 1)?;
            let right = argument_subject(program, binary.right, depth + 1)?;
            if !left.is_valid() || left == right {
                Some(right)
            } else if !right.is_valid() {
                Some(left)
            } else {
                None
            }
        }
        _ => None,
    }
}
