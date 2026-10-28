//! Solve all reachable normal exits in the declared slot namespace.
//! Completed operands retain their exact source cuts and carrier/backing association.

use super::super as frames;
use super::*;

pub(in crate::machine_calls::calls::write_frames) fn state_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    if inference.active_states.contains(&entry.symbol) {
        return None;
    }
    inference.active_states.push(entry.symbol);
    let result = solve_state_effects(program, machine, entry, symbols, inference);
    inference.active_states.pop();
    result
}

enum SlotExit {
    Return(Vec<StoredLocalOrigins>),
    Edge {
        target: usize,
        arguments: Vec<SlotArgumentCapture>,
        prefix: CollectedStatementPrefix,
    },
}

struct SlotEquation<'program> {
    state: &'program State,
    inventory: Vec<SlotInventory>,
    exits: Vec<SlotExit>,
}

/// Least fixed point over normal exits. An unvisited return is not an identity
/// route: only a real normal exit can seed incoming contents. Root substitution
/// finiteness bounds the source set before iteration; there is no depth cutoff.
fn solve_state_effects<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    entry: &'program State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    let mut states = vec![entry];
    let mut equations = Vec::new();
    let mut substitutions = frames::cycle_substitutions::Substitutions::default();
    let mut index = 0;
    while index < states.len() {
        let state = states[index];
        let inventory = destination_inventory(program, state)?;
        let (last, prefixes) =
            stored_reference_transfer(program, machine, state, symbols, inference)?;
        let mut exits = Vec::new();
        let mut has_transition = false;
        for (ordinal, statement) in program
            .statement_table
            .indexed_statements(state.statement_nodes)
        {
            let Some(transition) = statement.as_transition() else {
                continue;
            };
            has_transition = true;
            if transition.exit != TransitionExit::Ordinary {
                continue;
            }
            let prefix = prefixes.get(ordinal)?.as_ref()?;
            let completed = expression_effects::completed_targets(
                program, machine, state, statement, prefix, symbols, inference,
            )?;
            for edge in transition.targets() {
                let completed = completed
                    .iter()
                    .find(|completed| completed.target == edge)?;
                match program.statement_table.transition_target(edge) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        if frames::terminal_calls::target(program, machine, edge).is_valid() {
                            return None;
                        }
                        let target = frames::transition_topology::named_transition_target_state(
                            program, machine, state, edge,
                        )?;
                        let target_index = if let Some(index) = states
                            .iter()
                            .position(|candidate| candidate.symbol == target.symbol)
                        {
                            index
                        } else {
                            states.push(target);
                            states.len() - 1
                        };
                        let arguments = program.statement_table.expression_handles(*arguments);
                        let parameters = program.state_parameters(target);
                        if parameters.len() != arguments.len() {
                            return None;
                        }
                        if completed.arguments.len() != arguments.len()
                            || completed
                                .arguments
                                .iter()
                                .zip(arguments)
                                .any(|(captured, expression)| captured.expression != *expression)
                        {
                            return None;
                        }
                        exits.push(SlotExit::Edge {
                            target: target_index,
                            arguments: completed.arguments.clone(),
                            prefix: completed.context.clone(),
                        });
                    }
                    TransitionTargetNode::Terminal | TransitionTargetNode::Value(_) => {
                        exits.push(SlotExit::Return(completed.context.stored.clone()))
                    }
                }
            }
        }
        if !has_transition {
            exits.push(SlotExit::Return(last.stored));
        }
        equations.push(SlotEquation {
            state,
            inventory,
            exits,
        });
        index += 1;
    }
    let mut known = vec![None::<Vec<SlotEffect>>; equations.len()];
    loop {
        let mut changed = false;
        for (index, equation) in equations.iter().enumerate() {
            let mut candidate = Vec::new();
            let mut returned = Vec::new();
            for exit in &equation.exits {
                let stored = match exit {
                    SlotExit::Return(stored) => stored.clone(),
                    SlotExit::Edge {
                        target,
                        arguments,
                        prefix,
                    } => {
                        let Some(relative) = &known[*target] else {
                            continue;
                        };
                        let instantiated = instantiate(
                            program,
                            machine,
                            program.state_parameters(equations[*target].state),
                            arguments,
                            relative,
                            prefix,
                            symbols,
                        )?;
                        retain_demanded_substitutions(
                            equation.state,
                            equations[*target].state,
                            &instantiated.substitutions,
                            &mut substitutions,
                        )?;
                        let mut stored = prefix.stored.clone();
                        apply(program, &mut stored, &instantiated.effects);
                        stored
                    }
                };
                returned.push(stored);
            }
            if returned.is_empty() {
                continue;
            }
            // A slice's type inventory is a default for every element. Exact
            // writes refine that inventory, and every exit must answer every
            // refined slot. Otherwise an unchanged branch would disappear
            // when another branch replaces one element.
            let mut inventory = equation.inventory.clone();
            for declaration in &equation.inventory {
                for stored in &returned {
                    let record = stored
                        .iter()
                        .find(|record| record.local_symbol == declaration.origin.local_symbol)?;
                    for leaf in &record.references {
                        if leaf.local_segments.len() != declaration.origin.local_segments.len()
                            || !(stored_origins::path_covers(
                                &declaration.origin.local_segments,
                                &leaf.local_segments,
                            ) || (leaf.may_select
                                && stored_origins::path_covers(
                                    &leaf.local_segments,
                                    &declaration.origin.local_segments,
                                )))
                            || inventory.iter().any(|existing| {
                                existing.origin.local_symbol == leaf.local_symbol
                                    && existing.origin.local_segments == leaf.local_segments
                                    && existing.origin.may_select == leaf.may_select
                            })
                        {
                            continue;
                        }
                        let mut refined = declaration.clone();
                        refined.origin.local_segments = leaf.local_segments.clone();
                        refined.origin.may_select = leaf.may_select;
                        for ancestor in &mut refined.ancestors {
                            for (segment, selected) in ancestor.iter_mut().zip(&leaf.local_segments)
                            {
                                if matches!(segment, PlaceSegment::Index { expression } if !expression.is_valid())
                                {
                                    *segment = *selected;
                                }
                            }
                        }
                        inventory.push(refined);
                    }
                }
            }
            for stored in &returned {
                collect_exit(program, equation.state, &inventory, stored, &mut candidate)?;
            }
            if known[index].is_none() {
                known[index] = Some(Vec::new());
                changed = true;
            }
            let accumulated = known[index].as_mut()?;
            for effect in candidate {
                if let Some(prior) = accumulated.iter_mut().find(|prior| {
                    prior.root == effect.root
                        && prior.segments == effect.segments
                        && prior.may_select == effect.may_select
                }) {
                    if prior.kind != effect.kind {
                        return None;
                    }
                    if effect.incoming && !prior.incoming {
                        prior.incoming = true;
                        changed = true;
                    }
                    let prior_sources = prior.sources.as_mut()?;
                    for source in effect.sources? {
                        if !prior_sources.contains(&source) {
                            prior_sources.push(source);
                            changed = true;
                        }
                    }
                } else {
                    accumulated.push(effect);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    known.into_iter().next()?
}

/// Only actual relation roots participate in substitution. A checked empty
/// relation demands no argument origin; an unknown relation never reaches here.
/// Ordered argument effects have already established each selected edge's slot
/// inventory. The same growing-cycle law still bounds every demanded root.
fn retain_demanded_substitutions(
    source: &State,
    target: &State,
    demanded: &[(SymbolHandle, FrameSourcePlace)],
    substitutions: &mut frames::cycle_substitutions::Substitutions,
) -> Option<()> {
    for (parameter, actual) in demanded {
        if !actual.builtin_coordinates {
            return None;
        }
        if actual.is_static() {
            continue;
        }
        if !actual.root.is_valid() {
            return None;
        }
        let from = substitutions.parameter(target.symbol, *parameter);
        let to = substitutions.parameter(source.symbol, actual.root);
        substitutions.insert(
            from,
            to,
            !actual.segments.is_empty() || !actual.recasts.is_empty(),
        );
    }
    substitutions.is_finite().then_some(())
}

fn collect_exit(
    program: &dyn crate::CheckingInput,
    state: &State,
    inventory: &[SlotInventory],
    stored: &[StoredLocalOrigins],
    effects: &mut Vec<SlotEffect>,
) -> Option<()> {
    for declaration in inventory {
        let slot = &declaration.origin;
        // Runtime selector identity cannot escape its expression namespace.
        // A type-derived wildcard is different: it records incoming contents
        // for untouched elements, beside independently checked exact overlays.
        if slot
            .local_segments
            .iter()
            .any(|part| matches!(part, PlaceSegment::Index { expression } if expression.is_valid()))
        {
            return None;
        }
        let record = stored
            .iter()
            .find(|record| record.local_symbol == slot.local_symbol)?;
        for ancestor in &declaration.ancestors {
            let matching = stored_origins::slot_origins(record, ancestor)?;
            let origin = matching.first()?;
            if matching.len() != 1
                || origin.source.root != slot.local_symbol
                || &origin.source.segments != ancestor
                || !origin.source.builtin_coordinates
            {
                return None;
            }
        }
        let sources = stored_origins::selected_slot_sources(
            record,
            &slot.local_segments,
            Some(slot.may_select),
        );
        // A possible store on another exit need not occur on this one. Its
        // presence never kills an incoming alternative on either route.
        let sources = match sources {
            Some(sources) => sources,
            None if slot.may_select
                && !record
                    .references
                    .iter()
                    .any(|leaf| leaf.may_select && leaf.local_segments == slot.local_segments) =>
            {
                continue;
            }
            None => return None,
        };
        for supplied in sources {
            let source = &supplied.origin;
            if !source.source.builtin_coordinates {
                return None;
            }
            if !source.source.is_static() && source.source.output_identity().is_none() {
                if !source.source.root.is_valid() {
                    return None;
                }
                let parameter = program.state_parameter_by_symbol(state, source.source.root)?;
                // Owned input storage dies in the callee. Only a reference
                // boundary actually crossed by this source survives invocation.
                if !frames::type_reference_is_reference(program, parameter.type_reference) {
                    let frontier = stored_origins::declared_origins_for_query(
                        program,
                        parameter.symbol,
                        parameter.name.as_str(),
                        parameter.type_reference,
                        true,
                    )?;
                    if !frontier.references.iter().any(|leaf| {
                        stored_origins::source_reaches_leaf(
                            &source.source.segments,
                            &leaf.local_segments,
                        )
                    }) {
                        return None;
                    }
                }
            }
            let incoming = !supplied.lifetime_access.is_some()
                && source.source.root == slot.local_symbol
                && source.source.segments == slot.local_segments;
            if !incoming
                && !slot.may_select
                && slot
                    .local_segments
                    .iter()
                    .any(|part| matches!(part, PlaceSegment::Index { .. }))
            {
                return None;
            }
            let effect = if let Some(index) = effects.iter().position(|effect| {
                effect.root == slot.local_symbol
                    && effect.segments == slot.local_segments
                    && effect.may_select == slot.may_select
            }) {
                &mut effects[index]
            } else {
                effects.push(SlotEffect {
                    root: slot.local_symbol,
                    segments: slot.local_segments.clone(),
                    sources: Some(Vec::new()),
                    incoming: slot.may_select,
                    may_select: slot.may_select,
                    kind: stored_origins::StoredOriginKind::Reference(declaration.access),
                });
                effects.last_mut()?
            };
            if effect.kind != stored_origins::StoredOriginKind::Reference(declaration.access) {
                return None;
            }
            effect.incoming |= incoming;
            let sources = effect.sources.as_mut()?;
            if !incoming && !sources.contains(&supplied) {
                sources.push(supplied);
            }
        }
    }
    Some(())
}
