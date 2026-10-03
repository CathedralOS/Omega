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
    program: &TypedTrees,
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
        kind: Default::default(),
        root: backing,
        segments: Vec::new(),
        builtin_coordinates: true,
    }];
    let slots = transferred_slots(
        program,
        machine,
        target,
        arguments,
        prefix,
        &initial,
        symbols,
        &mut inference,
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
        let state = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == arrival.state)?;
        let (last, prefixes) =
            stored_reference_transfer(program, machine, state, symbols, &mut inference)?;
        let mut has_transition = false;
        for (index, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
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
            for edge in [transition.target, transition.continuation] {
                if !edge.is_valid() {
                    continue;
                }
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
                let next = Arrival {
                    state: target.symbol,
                    slots: transferred_slots(
                        program,
                        machine,
                        target,
                        arguments,
                        prefix,
                        &arrival.slots,
                        symbols,
                        &mut inference,
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
    program: &TypedTrees,
    state: &State,
    stored: &[StoredLocalOrigins],
    slots: &[FrameSourcePlace],
) -> Option<()> {
    for destination in destination_inventory(program, state)? {
        let record = stored
            .iter()
            .find(|record| record.local_symbol == destination.local_symbol)?;
        let mut leaves = record
            .references
            .iter()
            .filter(|leaf| leaf.local_segments == destination.local_segments)
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
    program: &TypedTrees,
    machine: &Machine,
    target: &State,
    arguments: &[ExpressionHandle],
    prefix: &CollectedStatementPrefix,
    carried: &[FrameSourcePlace],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
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
        let captured = argument_origin(
            program,
            machine,
            parameter,
            *arguments.get(index)?,
            prefix,
            symbols,
            inference,
        )?;
        for segments in slots {
            let slot = FrameSourcePlace {
                kind: Default::default(),
                root: parameter.symbol,
                segments,
                builtin_coordinates: true,
            };
            let mut projected = captured.clone();
            projected.source = captured.source.append_relative(&slot);
            projected.path = if projected.source.is_static() {
                String::new()
            } else {
                path(program, &projected.source)
            };
            let origins = stored_origins::captured_reference_projection_origins(
                program,
                &captured,
                &projected,
                &prefix.stored,
            );
            let mut contains = false;
            for origin in origins {
                contains |= carries(&origin?, carried)?;
            }
            if contains && !selected.contains(&slot) {
                selected.push(slot);
            }
        }
    }
    Some(selected)
}
