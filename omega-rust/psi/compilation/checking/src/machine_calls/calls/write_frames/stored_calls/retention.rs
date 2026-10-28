//! A state edge may retain a local home through a stored reference. Follow the
//! exact carried source through every successor slot/copy until replacement or
//! last transfer. A normal exit cannot export it, and a loop cannot establish
//! the same home again while an earlier generation is still carried.

use super::*;

#[derive(Clone, PartialEq, Eq)]
struct Arrival {
    state: SymbolHandle,
    slots: Vec<FrameSourcePlace>,
}

pub(in crate::machine_calls::calls::write_frames) fn local_backing_survives_jump(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    source: &State,
    statement_index: usize,
    target: &State,
    arguments: &[ExpressionHandle],
    backing: SymbolHandle,
    symbols: &TopLevelSymbols<'_>,
) -> Option<()> {
    program.state_local_declaration(source, backing)?;
    let mut inference = FrameInference::default();
    let (_, prefixes) =
        stored_reference_transfer(program, machine, source, symbols, &mut inference)?;
    let prefix = prefixes.get(statement_index)?.as_ref()?;
    let initial = vec![FrameSourcePlace {
        recasts: Vec::new(),
        representation: None,
        kind: Default::default(),
        root: backing,
        segments: Vec::new(),
        builtin_coordinates: true,
    }];
    let statement = program
        .statement_table
        .statements(source.statement_nodes)
        .get(statement_index)?;
    let completed = expression_effects::completed_targets(
        program,
        machine,
        source,
        statement,
        prefix,
        symbols,
        &mut inference,
    )?;
    let mut arrivals = completed.iter().filter(|completed| {
        super::super::transition_topology::named_transition_target_state(
            program,
            machine,
            source,
            completed.target,
        )
        .is_some_and(|candidate| candidate.symbol == target.symbol)
            && completed.arguments.len() == arguments.len()
            && completed
                .arguments
                .iter()
                .zip(arguments)
                .all(|(captured, expression)| captured.expression == *expression)
    });
    let arrival = arrivals.next()?;
    if arrivals.next().is_some() {
        return None;
    }
    let slots = transferred_slots(
        program,
        machine,
        target,
        &arrival.arguments,
        &arrival.context,
        &initial,
        symbols,
    )?;
    let mut pending = vec![Arrival {
        state: target.symbol,
        slots,
    }];
    let mut cursor = 0;
    while cursor < pending.len() {
        let arrival = pending[cursor].clone();
        cursor += 1;
        if arrival.slots.is_empty() {
            continue;
        }
        let state = program.state_in_machine(machine, arrival.state)?;
        let (last, prefixes) =
            stored_reference_transfer(program, machine, state, symbols, &mut inference)?;
        let mut has_transition = false;
        for (index, statement) in program
            .statement_table
            .indexed_statements(state.statement_nodes)
        {
            let prefix = prefixes.get(index)?.as_ref()?;
            // Birth is a write to the physical home, even for plain scalar or
            // array storage which owes no owned cleanup action.
            if matches!(statement, StatementNode::LocalData(local) if local.symbol == backing)
                && prefix_carries(prefix, &arrival.slots)?
            {
                return None;
            }
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            has_transition = true;
            if transition.exit != TransitionExit::Ordinary {
                continue;
            }
            let completed = expression_effects::completed_targets(
                program,
                machine,
                state,
                statement,
                prefix,
                symbols,
                &mut inference,
            )?;
            for edge in transition.targets() {
                let edge_value = completed
                    .iter()
                    .find(|completed| completed.target == edge)?;
                let (target, arguments) = match program.statement_table.transition_target(edge) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        if super::super::terminal_calls::target(program, machine, edge).is_valid() {
                            return None;
                        }
                        (
                            super::super::transition_topology::named_transition_target_state(
                                program, machine, state, edge,
                            )?,
                            program.statement_table.expression_handles(*arguments),
                        )
                    }
                    TransitionTargetNode::Terminal | TransitionTargetNode::Value(_) => {
                        check_exit(program, state, &prefix.stored, &arrival.slots)?;
                        continue;
                    }
                };
                if edge_value.arguments.len() != arguments.len()
                    || edge_value
                        .arguments
                        .iter()
                        .zip(arguments)
                        .any(|(captured, expression)| captured.expression != *expression)
                {
                    return None;
                }
                let next = Arrival {
                    state: target.symbol,
                    slots: transferred_slots(
                        program,
                        machine,
                        target,
                        &edge_value.arguments,
                        &edge_value.context,
                        &arrival.slots,
                        symbols,
                    )?,
                };
                if !next.slots.is_empty() && !pending.contains(&next) {
                    pending.push(next);
                }
            }
        }
        if !has_transition {
            check_exit(program, state, &last.stored, &arrival.slots)?;
        }
    }
    Some(())
}

fn carries(origin: &FramePlaceOrigin, slots: &[FrameSourcePlace]) -> Option<bool> {
    if origin.source.is_static() {
        return Some(false);
    }
    if !origin.source.root.is_valid() || !origin.source.builtin_coordinates {
        return None;
    }
    Some(slots.iter().any(|slot| {
        origin.source.root == slot.root
            && stored_origins::source_reaches_leaf(&origin.source.segments, &slot.segments)
    }))
}

fn prefix_carries(prefix: &CollectedStatementPrefix, slots: &[FrameSourcePlace]) -> Option<bool> {
    let mut carried = false;
    for (_, origin) in &prefix.aliases {
        carried |= carries(origin, slots)?;
    }
    for (_, origins) in &prefix.divergent {
        for origin in origins {
            carried |= carries(origin, slots)?;
        }
    }
    for record in &prefix.stored {
        for leaf in &record.references {
            carried |= carries(leaf.origin.as_ref()?, slots)?;
        }
    }
    // A direct input reference is itself a carrier; it is not a stored leaf.
    // It remains an arrival value for this state's body.
    carried |= slots.iter().any(|slot| slot.segments.is_empty());
    Some(carried)
}

fn check_exit(
    program: &dyn crate::CheckingInput,
    state: &State,
    stored: &[StoredLocalOrigins],
    slots: &[FrameSourcePlace],
) -> Option<()> {
    for declaration in destination_inventory(program, state)? {
        let destination = declaration.origin;
        let record = stored
            .iter()
            .find(|record| record.local_symbol == destination.local_symbol)?;
        let mut leaves = record
            .references
            .iter()
            .filter(|leaf| {
                leaf.local_segments.len() == destination.local_segments.len()
                    && (stored_origins::path_covers(
                        &destination.local_segments,
                        &leaf.local_segments,
                    ) || (leaf.may_select
                        && stored_origins::path_covers(
                            &leaf.local_segments,
                            &destination.local_segments,
                        )))
            })
            .peekable();
        leaves.peek()?;
        for leaf in leaves {
            if carries(leaf.origin.as_ref()?, slots)? {
                return None;
            }
        }
    }
    Some(())
}

fn transferred_slots(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    target: &State,
    arguments: &[SlotArgumentCapture],
    prefix: &CollectedStatementPrefix,
    carried: &[FrameSourcePlace],
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<FrameSourcePlace>> {
    let parameters = program.state_parameters(target);
    if arguments.len() != parameters.len() || !prefix.divergent.is_empty() {
        return None;
    }
    let mut selected = Vec::new();
    for (index, parameter) in parameters.iter().enumerate() {
        let mut slots = Vec::new();
        let reference =
            super::super::type_reference_is_reference(program, parameter.type_reference);
        if reference {
            slots.push(Vec::new());
        }
        let storage = if reference {
            super::super::reference_subjects::carrier_storage_type(
                program,
                parameter.type_reference,
            )?
        } else {
            parameter.type_reference
        };
        for leaf in stored_origins::declared_origins_for_query(
            program,
            parameter.symbol,
            parameter.name.as_str(),
            storage,
            true,
        )?
        .references
        {
            if leaf
                .local_segments
                .iter()
                .any(|segment| matches!(segment, PlaceSegment::Index { .. }))
            {
                return None;
            }
            slots.push(leaf.local_segments);
        }
        if slots.is_empty() {
            continue;
        }
        for segments in slots {
            let slot = FrameSourcePlace {
                recasts: Vec::new(),
                representation: None,
                kind: Default::default(),
                segments,
                ..FrameSourcePlace::parameter_root(parameter)
            };
            let mut contains = false;
            for source in aggregate_argument_sources(
                program,
                machine,
                parameter,
                arguments.get(index)?,
                &SlotSource::from_place(program, slot.clone(), FramePathPrecision::Exact, None),
                prefix,
                symbols,
            )? {
                contains |= carries(&source.origin, carried)?;
            }
            if contains && !selected.contains(&slot) {
                selected.push(slot);
            }
        }
    }
    Some(selected)
}
