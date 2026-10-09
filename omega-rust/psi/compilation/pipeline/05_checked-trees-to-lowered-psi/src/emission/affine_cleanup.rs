//! Complete no-code affine carrier custody after a body's operation graph exists.
//!
//! Callers supply source-checked no-code disposal capability independently of
//! any scheduled death. The actual operation graph supplies births and selected
//! transfers; their finite frontier equations settle every normal predecessor
//! before a join. View and reference carriers retain their separate loan cleanup
//! authority and die before their backing owners.
//! Carrier disposal grants no ownership of borrowed backing storage. Borrowing
//! a carrier across an edge retains its original disposal debt until exit.
//! Region disposal retains its exact checked action stream independently of
//! whole-home liveness; explicit retirement ends that home's current epoch.
//! Cycles preserve the same finite carrier frontier; nominal cleanup remains
//! outside this pass.
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::{BlockId, StructuralPlaceKind};
use terminal_psi::{
    Block, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, TerminalAffineCleanupAction, Terminator,
};

mod custody_flow;
mod projected_transfers;

/// Join source-authorized no-code type capability to this closure's existing
/// normalized type identities. Terminal storage geometry alone cannot authorize
/// disposal, and a final death row cannot describe a skipped earlier transfer.
pub(crate) fn no_code_type_ids(
    checked: &checked_trees::CheckedTrees,
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
) -> Vec<semantic_vocabulary::StructuralTypeId> {
    type_ids
        .iter()
        .filter_map(|(identity, id)| {
            checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter()
                .any(|declaration| {
                    declaration.identity == *identity && declaration.no_code_affine_cleanup
                })
                .then_some(*id)
        })
        .collect()
}

/// Complete source-checked no-code custody in the emitted operation graph.
/// `parameters` delegates entry cleanup; the complete entry roster supplies
/// residual geometry. Only the checked capability roster authorizes additional
/// plain owned local/result and receiving-block homes. Actual affine access,
/// qualifications and claims remain properties of each occurrence, independently
/// of its type. Nominal cleanup retains its selected ordered action stream.
pub(crate) fn complete(
    parameters: &[StructuralParameterDeclaration],
    entry_parameters: &[StructuralParameterDeclaration],
    no_code_types: &[semantic_vocabulary::StructuralTypeId],
    local_places: &[StructuralPlaceDeclaration],
    entry: BlockId,
    blocks: &mut [Block],
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    complete_carriers(
        parameters,
        no_code_types,
        local_places,
        entry,
        blocks,
        types,
    )?;
    projected_transfers::complete(entry_parameters, entry, blocks, types)
}

fn complete_carriers(
    parameters: &[StructuralParameterDeclaration],
    no_code_types: &[semantic_vocabulary::StructuralTypeId],
    local_places: &[StructuralPlaceDeclaration],
    entry: BlockId,
    blocks: &mut [Block],
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    let affine = parameters
        .iter()
        .rev()
        .filter_map(|parameter| {
            (parameter.access == StructuralAccess::Owned
                && parameter.multiplicity == StructuralMultiplicity::Affine)
                .then_some(parameter.place)
        })
        .collect::<Vec<_>>();
    // Descriptor cleanup remains owned by its loan continuation. Plain owned
    // type capability is independent of this already scheduled death roster.
    let scheduled = blocks
        .iter()
        .flat_map(|block| scheduled_discards(&block.terminator))
        .collect::<Vec<_>>();
    // A partial transfer retains root storage and its independently selected
    // residual disposal. The whole-home solver must not replace that schedule.
    let mut partial_transfers = Vec::new();
    for block in blocks.iter() {
        let operands = block
            .operations
            .iter()
            .flat_map(|operation| projected_transfers::operands(&operation.kind))
            .chain(match &block.terminator {
                Terminator::Jump {
                    structural_arguments,
                    ..
                } => structural_arguments.iter().collect::<Vec<_>>(),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => when_true
                    .structural_arguments
                    .iter()
                    .chain(&when_false.structural_arguments)
                    .collect(),
                _ => Vec::new(),
            });
        for argument in operands {
            if argument.access != StructuralAccess::Owned {
                continue;
            }
            if !argument.path.is_empty() {
                partial_transfers.push(argument.place);
            }
        }
        for operation in &block.operations {
            if let terminal_psi::OperationKind::ReleaseReference { source, path } = &operation.kind
            {
                if !path.is_empty() {
                    partial_transfers.push(*source);
                }
            }
        }
    }
    let births = blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            let place = established_place(operation)?;
            if partial_transfers.contains(&place) {
                return None;
            }
            let Some(result) = operation.result.structural() else {
                // In-place establishment owns a declared local home, not an
                // operation result. Rejoin its exact checked type capability.
                return local_places.iter().find_map(|local| {
                    (local.id == place
                        && matches!(local.kind,
                            StructuralPlaceKind::TrivialAffineLocal { structural_type, .. }
                                if no_code_types.contains(&structural_type)))
                    .then_some((place, operation.id))
                });
            };
            if !result.is_affine_plain() || !result.qualification_establishments.is_empty() {
                return None;
            }
            let mut declarations = types
                .iter()
                .filter(|declaration| declaration.id == result.structural_type);
            let declaration = declarations.next()?;
            let unscheduled_carrier = !scheduled.contains(&result.place)
                && (matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ElementView { .. }
                        | terminal_psi::StructuralTypeShape::Reference { .. }
                ) || declaration.shape.is_borrowed_byte_view());
            if declarations.next().is_some()
                || !(no_code_types.contains(&result.structural_type) || unscheduled_carrier)
            {
                return None;
            }
            Some((result.place, operation.id))
        })
        .collect::<Vec<_>>();
    let mut managed = affine.clone();
    managed.extend(births.iter().map(|(place, _)| *place));
    // A prior owned move can give a source local a block-parameter home.
    // Its exact incoming edge is its birth, just as an operation owns a
    // constructed result's birth; neither home changes disposal semantics.
    managed.extend(
        blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
            .filter(|parameter| {
                parameter.is_unqualified_owned_affine()
                    && !affine.contains(&parameter.place)
                    && no_code_types.contains(&parameter.structural_type)
                    && !partial_transfers.contains(&parameter.place)
            })
            .map(|parameter| parameter.place),
    );
    if managed.is_empty() {
        return Ok(());
    }
    if managed
        .iter()
        .enumerate()
        .any(|(position, place)| managed[..position].contains(place))
    {
        return unsupported("scalar graph affine parameters have duplicate places");
    }
    let entry_position = block_position(blocks, entry)?;
    let reentered = !affine.is_empty() && !blocks[entry_position].structural_parameters.is_empty();
    let mut incoming = vec![Vec::new(); blocks.len()];
    let mut successors = vec![Vec::new(); blocks.len()];
    for (position, block) in blocks.iter().enumerate() {
        // Local record moves/cleanup use their own block homes. This pass owns
        // only the incoming formal frontier and must neither reject unrelated
        // local transport nor mistake it for forwarding one of these formals.
        if (position != entry_position || !reentered)
            && block
                .structural_parameters
                .iter()
                .any(|parameter| affine.contains(&parameter.place))
        {
            return unsupported(
                "scalar graph owned block parameters require exact forwarding custody",
            );
        }
        let targets = match &block.terminator {
            Terminator::Jump {
                target,
                structural_arguments,
                cleanup_actions,
                ..
            } => {
                if (structural_arguments.iter().any(|argument| {
                    affine.contains(&argument.place) && argument.access == StructuralAccess::Owned
                }) && !(reentered && *target == entry))
                    || cleanup_actions
                        .iter()
                        .any(|action| competing_cleanup(action, &affine, &managed))
                {
                    return unsupported(
                        "scalar graph whole parameter cleanup cannot replace existing edge custody",
                    );
                }
                if reentered && *target == entry && structural_arguments.len() != parameters.len() {
                    return unsupported(
                        "owned backedge lost its complete structural argument roster",
                    );
                }
                vec![*target]
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                if [when_true, when_false].iter().any(|edge| {
                    edge.structural_arguments.iter().any(|argument| {
                        affine.contains(&argument.place)
                            && argument.access == StructuralAccess::Owned
                    }) || edge
                        .cleanup_actions
                        .iter()
                        .any(|action| competing_cleanup(action, &affine, &managed))
                        || (reentered && edge.target == entry)
                }) {
                    return unsupported(
                        "scalar graph whole parameter cleanup cannot replace existing branch custody",
                    );
                }
                vec![when_true.target, when_false.target]
            }
            Terminator::StructuralCase { cases, .. } => {
                if cases.iter().any(|edge| {
                    edge.cleanup_actions
                        .iter()
                        .any(|action| competing_cleanup(action, &affine, &managed))
                        || (reentered && edge.target == entry)
                }) {
                    return unsupported(
                        "scalar graph whole parameter cleanup cannot replace existing branch custody",
                    );
                }
                cases.iter().map(|edge| edge.target).collect()
            }
            terminal_psi::return_or_return_structural_pattern!(cleanup_actions, ..)
            | Terminator::ReturnUnit {
                cleanup_actions, ..
            } if cleanup_actions
                .iter()
                .all(|action| !competing_cleanup(action, &affine, &managed)) =>
            {
                Vec::new()
            }
            // An abandoning exit has no successor and runs no parameter
            // cleanup: a crash, or an external completion such as process exit.
            Terminator::Crash { .. } | Terminator::ExternalCompletion { .. } => Vec::new(),
            _ => {
                return unsupported(
                    "scalar graph whole parameter cleanup requires scalar or Unit exits and ordinary edges",
                );
            }
        };
        for target in targets {
            let target_position = block_position(blocks, target)?;
            if !(reentered && target == entry) {
                incoming[target_position].push(position);
            }
            successors[position].push(target_position);
        }
    }
    if !incoming[entry_position].is_empty() {
        return unsupported(
            "cyclic owned scalar graphs require exact structural forwarding custody",
        );
    }
    let custody_flow::Frontiers {
        entries,
        exits,
        outgoing,
    } = custody_flow::solve(
        blocks,
        entry_position,
        &affine,
        &managed,
        &births,
        &incoming,
        &successors,
    )?;
    // A join keeps precisely the owners live on every incoming path. The
    // other paths dispose their surviving roots on their own selected edge.
    // A later read cannot revive a discarded owner; frontier verification
    // checks every operation against the reconciled entry.
    let producers = blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| established_place(operation).map(|place| (place, operation.id)))
        .collect::<Vec<_>>();
    let dominators =
        crate::emission::control_flow::block_dominators(Some(entry_position), &successors);
    let mut parameter_blocks = blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| !block.structural_parameters.is_empty())
        .collect::<Vec<_>>();
    parameter_blocks.sort_by_key(|(position, _)| dominators[*position].len());
    let parameter_order = parameters
        .iter()
        .map(|parameter| parameter.place)
        .chain(
            parameter_blocks
                .iter()
                .flat_map(|(_, block)| &block.structural_parameters)
                .map(|parameter| parameter.place),
        )
        .collect::<Vec<_>>();
    for (position, block) in blocks.iter_mut().enumerate() {
        let discards = |edge_position: usize, target_position: usize| {
            outgoing[position][edge_position]
                .iter()
                .copied()
                .filter(|place| !entries[target_position].contains(place))
                .collect::<Vec<_>>()
        };
        match &mut block.terminator {
            Terminator::Jump {
                target,
                structural_arguments,
                cleanup_actions,
                ..
            } => {
                if reentered && *target == entry {
                    // Cutting the backedge is only a construction schedule.
                    // Reconstruct its complete transfer from the live exit;
                    // every affine header owner must be reestablished exactly
                    // once before this edge can close the ownership fixed point.
                    let mut remaining = exits[position].clone();
                    for (parameter, argument) in parameters.iter().zip(structural_arguments) {
                        if argument.access != parameter.access || !argument.path.is_empty() {
                            return unsupported(
                                "owned backedge changes access or projects a whole owner",
                            );
                        }
                        if parameter.multiplicity == StructuralMultiplicity::Affine {
                            let Some(position) =
                                remaining.iter().position(|place| *place == argument.place)
                            else {
                                return unsupported(
                                    "owned backedge transfers a missing or already consumed owner",
                                );
                            };
                            remaining.remove(position);
                        }
                    }
                    replace_managed_discards(
                        cleanup_actions,
                        remaining,
                        &managed,
                        &producers,
                        &parameter_order,
                    );
                } else {
                    replace_managed_discards(
                        cleanup_actions,
                        discards(0, successors[position][0]),
                        &managed,
                        &producers,
                        &parameter_order,
                    );
                }
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                replace_managed_discards(
                    &mut when_true.cleanup_actions,
                    discards(0, successors[position][0]),
                    &managed,
                    &producers,
                    &parameter_order,
                );
                replace_managed_discards(
                    &mut when_false.cleanup_actions,
                    discards(1, successors[position][1]),
                    &managed,
                    &producers,
                    &parameter_order,
                );
            }
            Terminator::StructuralCase { cases, .. } => {
                for (edge_position, (edge, successor)) in
                    cases.iter_mut().zip(&successors[position]).enumerate()
                {
                    replace_managed_discards(
                        &mut edge.cleanup_actions,
                        discards(edge_position, *successor),
                        &managed,
                        &producers,
                        &parameter_order,
                    );
                }
            }
            terminal_psi::return_or_return_structural_pattern!(cleanup_actions, ..)
            | Terminator::ReturnUnit {
                cleanup_actions, ..
            } => {
                replace_managed_discards(
                    cleanup_actions,
                    exits[position].clone(),
                    &managed,
                    &producers,
                    &parameter_order,
                );
            }
            _ => {}
        }
    }
    Ok(())
}

/// Recompute only the disposal this invocation owns. Unrelated statement,
/// residual, claim-bearing and nominal obligations keep their existing owner.
fn replace_managed_discards(
    existing: &mut Vec<TerminalAffineCleanupAction>,
    additions: Vec<semantic_vocabulary::PlaceId>,
    managed: &[semantic_vocabulary::PlaceId],
    producers: &[(
        semantic_vocabulary::PlaceId,
        semantic_vocabulary::OperationId,
    )],
    parameter_order: &[semantic_vocabulary::PlaceId],
) {
    // A region disposal retains its home, and explicit retirement already
    // ends that home's current epoch. Neither belongs to root-discard rewriting.
    // In particular replacement can dispose a field before a later selected
    // whole transfer of the same home; preserve both ownership operations.
    let additions = additions
        .into_iter()
        .filter(|place| {
            !existing.iter().any(|action| {
                matches!(action, TerminalAffineCleanupAction::RetireHome(retired) if retired == place)
            })
        })
        .collect();
    existing.retain(|action| {
        !matches!(action, TerminalAffineCleanupAction::DiscardRoot(place) if managed.contains(place))
    });
    merge_discards(existing, additions, producers, parameter_order);
}

/// Regional cleanup may coexist with a whole-home lifetime equation. Nominal
/// cleanup retains its independent ordered owner; delegated entry root disposal
/// must still be supplied exactly once by this invocation.
fn competing_cleanup(
    action: &TerminalAffineCleanupAction,
    affine: &[semantic_vocabulary::PlaceId],
    managed: &[semantic_vocabulary::PlaceId],
) -> bool {
    match action {
        TerminalAffineCleanupAction::DiscardRoot(place) => affine.contains(place),
        TerminalAffineCleanupAction::InvokeNominal(cleanup) => managed.contains(&cleanup.place),
        TerminalAffineCleanupAction::DiscardResidual(_)
        | TerminalAffineCleanupAction::RetireHome(_) => false,
    }
}

/// Runtime establishment identity, independent of whether the operation
/// returns the carrier or establishes a predeclared no-code local in place.
fn established_place(operation: &terminal_psi::Operation) -> Option<semantic_vocabulary::PlaceId> {
    if let Some(result) = operation.result.structural() {
        return Some(result.place);
    }
    match &operation.kind {
        terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination } => {
            Some(*destination)
        }
        _ => None,
    }
}

fn block_position(blocks: &[Block], target: BlockId) -> Result<usize, LoweringError> {
    let mut positions = blocks
        .iter()
        .enumerate()
        .filter_map(|(position, block)| (block.id == target).then_some(position));
    let position = positions.next().ok_or(LoweringError::Unsupported(
        "owned scalar graph has an absent block",
    ))?;
    if positions.next().is_some() {
        return unsupported("owned scalar graph has duplicate block identities");
    }
    Ok(position)
}

/// Insert new result cleanup in reverse producer order without rearranging an
/// already scheduled statement cleanup. Entry parameters follow local results.
fn merge_discards(
    existing: &mut Vec<TerminalAffineCleanupAction>,
    additions: Vec<semantic_vocabulary::PlaceId>,
    producers: &[(
        semantic_vocabulary::PlaceId,
        semantic_vocabulary::OperationId,
    )],
    parameter_order: &[semantic_vocabulary::PlaceId],
) {
    for place in additions {
        let insertion = disposal_position(
            existing.iter().map(TerminalAffineCleanupAction::place),
            place,
            producers,
            parameter_order,
        );
        existing.insert(insertion, TerminalAffineCleanupAction::DiscardRoot(place));
    }
}

fn disposal_position(
    existing: impl Iterator<Item = semantic_vocabulary::PlaceId>,
    place: semantic_vocabulary::PlaceId,
    producers: &[(
        semantic_vocabulary::PlaceId,
        semantic_vocabulary::OperationId,
    )],
    parameter_order: &[semantic_vocabulary::PlaceId],
) -> usize {
    let producer = |place| {
        producers
            .iter()
            .find_map(|(candidate, operation)| (*candidate == place).then_some(*operation))
    };
    let birth = producer(place);
    let parameter_position = parameter_order
        .iter()
        .rposition(|candidate| *candidate == place);
    let mut length = 0;
    for (position, current) in existing.enumerate() {
        if birth.is_some_and(|birth| {
            producer(current).is_none_or(|current_birth| current_birth < birth)
        }) || (birth.is_none()
            && parameter_position.is_some_and(|parameter_position| {
                parameter_order
                    .iter()
                    .rposition(|candidate| *candidate == current)
                    .is_some_and(|current_position| current_position < parameter_position)
            }))
        {
            return position;
        }
        length = position + 1;
    }
    length
}

fn scheduled_discards(terminator: &Terminator) -> Vec<semantic_vocabulary::PlaceId> {
    match terminator {
        Terminator::Jump {
            cleanup_actions, ..
        } => cleanup_actions
            .iter()
            .filter_map(|action| match action {
                TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                _ => None,
            })
            .collect(),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => when_true
            .cleanup_actions
            .iter()
            .chain(&when_false.cleanup_actions)
            .filter_map(|action| match action {
                TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                _ => None,
            })
            .collect(),
        terminal_psi::return_or_return_structural_pattern!(cleanup_actions, ..)
        | Terminator::ReturnUnit {
            cleanup_actions, ..
        } => cleanup_actions
            .iter()
            .filter_map(|action| match action {
                TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                _ => None,
            })
            .collect(),
        Terminator::StructuralCase { cases, .. } => cases
            .iter()
            .flat_map(|edge| &edge.cleanup_actions)
            .filter_map(|action| match action {
                TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}
