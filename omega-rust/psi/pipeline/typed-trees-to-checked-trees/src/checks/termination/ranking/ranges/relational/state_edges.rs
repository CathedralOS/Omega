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
    let scalar_subject = match measure {
        validation::RankingRangeMeasure::Single(subject)
        | validation::RankingRangeMeasure::IncreasingTo { subject, .. } => {
            match program.expression_table.expression(subject) {
                ExpressionNode::Name(name)
                    if name.symbol.is_valid() && name.head_symbol == name.symbol =>
                {
                    name.symbol
                }
                _ => SymbolHandle::default(),
            }
        }
        _ => SymbolHandle::default(),
    };
    let Some(mappings) = discover_mappings(program, machine, &adjacency, scalar_subject) else {
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

/// Identity transfers anchor the first closure. Computations can then establish
/// remaining telescopes from one dependency or one current representative of
/// the already-authored scalar rank subject, without selecting a new witness.
/// Each state enters the worklist once per tier. Every eligible incoming edge
/// checks its proposal, including edges to already-processed destinations, so
/// provisional discovery order cannot resolve conflicting correspondences.
fn discover_mappings(
    program: &TypedTrees,
    machine: &Machine,
    adjacency: &[Vec<usize>],
    scalar_subject: SymbolHandle,
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
                    let Some(incoming) = argument_mapping(
                        program,
                        machine,
                        source,
                        target,
                        &source_mapping,
                        edge.arguments,
                        scalar_subject,
                    ) else {
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
/// query independently proves equality among required scalar copies on every
/// arrival; discovering shared ancestry does not establish equal values.
fn argument_mapping(
    program: &TypedTrees,
    machine: &Machine,
    source: &State,
    target: &State,
    source_mapping: &[SymbolHandle],
    arguments: &[ExpressionHandle],
    scalar_subject: SymbolHandle,
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
    let mut subjects = Vec::new();
    for argument in arguments {
        subjects.clear();
        argument_subjects(program, machine, source, *argument, &mut subjects, 0)?;
        let mut selected_position = None;
        for subject in &subjects {
            let source_position = source_parameters.iter().position(|parameter| {
                parameter.symbol == *subject && !parameter.is_mutable && !parameter.is_const
            })?;
            if subjects.len() == 1 || source_mapping[source_position] == scalar_subject {
                // Count current representatives, not just root ancestry. Two
                // copies may have diverged and cannot choose each other's role.
                if selected_position.replace(source_position).is_some() {
                    return None;
                }
            }
        }
        let entry_symbol = source_mapping[selected_position?];
        parameters.push(entry_symbol);
    }
    Some(parameters)
}

/// Discover a dependency, not a value equality or an arithmetic theorem. The
/// ordinary edge query still checks selected builtin meaning and every rank
/// obligation before this provisional correspondence can authorize anything.
/// Retain all distinct current dependencies before selecting the authored role;
/// nested auxiliary arithmetic must not select an operand merely by position.
/// A literal subtree contributes no subject of its own.
fn argument_subjects(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    subjects: &mut Vec<SymbolHandle>,
    depth: usize,
) -> Option<()> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => {}
        ExpressionNode::Name(name) if name.symbol.is_valid() && name.head_symbol == name.symbol => {
            if !subjects.contains(&name.symbol) {
                subjects.push(name.symbol);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            argument_subjects(program, machine, state, atomic.value, subjects, depth + 1)?;
        }
        ExpressionNode::Indexed(indexed)
            if validation::has_builtin_subslice_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) =>
        {
            // A window retains its collection's lineage, not the lineage of
            // the scalar bounds. The edge query separately proves its length
            // and bounds before this mapping can support a ranking fact.
            argument_subjects(
                program,
                machine,
                state,
                indexed.collection,
                subjects,
                depth + 1,
            )?;
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Modulo
            ) =>
        {
            argument_subjects(program, machine, state, binary.left, subjects, depth + 1)?;
            argument_subjects(program, machine, state, binary.right, subjects, depth + 1)?;
        }
        _ => return None,
    }
    Some(())
}
