//! Complete no-code affine carrier custody after a body's operation graph exists.
//!
//! Callers supply only entry owners whose cleanup they delegate here. Existing
//! statement cleanup remains authoritative; otherwise unhandled borrowed-view
//! carriers enter at their result birth and are disposed before their owners.
//! Carrier disposal grants no ownership of borrowed backing storage. Borrowing
//! a carrier across an edge retains its original disposal debt until exit.
//! Cyclic descriptor forwarding and nominal cleanup remain outside this pass.
use crate::direct_calls::call_structural_arguments;
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::BlockId;
use std::collections::VecDeque;
use terminal_psi::{
    Block, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    TerminalAffineCleanupAction, Terminator,
};

mod projected_transfers;

#[cfg(test)]
mod tests;

/// The caller must first establish source no-code signature eligibility through
/// parameter_storage -> owned::validate, including permissions and exclusion of
/// nominal cleanup. Terminal multiplicity alone does not establish eligibility.
/// Pass an empty delegated roster when another emitter owns entry cleanup.
/// `entry_parameters` always retains the complete machine roster for exact
/// root types and declaration-ordered residual cleanup.
/// This pass completes the delegated custody through calls and continuations;
/// the independent verifier checks types, reads, and disposal completeness.
pub(crate) fn complete(
    parameters: &[StructuralParameterDeclaration],
    entry_parameters: &[StructuralParameterDeclaration],
    entry: BlockId,
    blocks: &mut [Block],
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    complete_carriers(parameters, entry, blocks, types)?;
    projected_transfers::complete(entry_parameters, entry, blocks, types)
}

fn complete_carriers(
    parameters: &[StructuralParameterDeclaration],
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
    // Existing statement emitters own every cleanup they already scheduled.
    // Discover unhandled carrier results by declared storage semantics, never by
    // the operation spelling which happened to create the descriptor.
    let scheduled = blocks
        .iter()
        .flat_map(|block| scheduled_discards(&block.terminator))
        .collect::<Vec<_>>();
    let births = blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            let result = operation.result.structural()?;
            if result.multiplicity != StructuralMultiplicity::Affine
                || scheduled.contains(&result.place)
                || !result.claims.is_empty()
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
            {
                return None;
            }
            let mut declarations = types
                .iter()
                .filter(|declaration| declaration.id == result.structural_type);
            let declaration = declarations.next()?;
            if declarations.next().is_some()
                || !(matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ElementView { .. }
                ) || declaration.shape.is_borrowed_byte_view())
            {
                return None;
            }
            Some((result.place, operation.id))
        })
        .collect::<Vec<_>>();
    let mut managed = affine.clone();
    managed.extend(births.iter().map(|(place, _)| *place));
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
    if reentered && blocks[entry_position].structural_parameters != parameters {
        return unsupported("owned loop entry differs from its allocated parameter frontier");
    }
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
                .any(|parameter| managed.contains(&parameter.place))
        {
            return unsupported(
                "scalar graph owned block parameters require exact forwarding custody",
            );
        }
        let targets = match &block.terminator {
            Terminator::Jump {
                target,
                structural_arguments,
                residual_affine_discards,
                trivial_affine_discards,
                ..
            } => {
                if (structural_arguments.iter().any(|argument| {
                    managed.contains(&argument.place) && argument.access == StructuralAccess::Owned
                }) && !(reentered && *target == entry))
                    || !residual_affine_discards.is_empty()
                    || trivial_affine_discards
                        .iter()
                        .any(|place| affine.contains(place))
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
                        managed.contains(&argument.place)
                            && argument.access == StructuralAccess::Owned
                    }) || edge
                        .trivial_affine_discards
                        .iter()
                        .any(|place| affine.contains(place))
                        || (reentered && edge.target == entry)
                }) {
                    return unsupported(
                        "scalar graph whole parameter cleanup cannot replace existing branch custody",
                    );
                }
                vec![when_true.target, when_false.target]
            }
            Terminator::Return {
                cleanup_actions, ..
            } if cleanup_actions.iter().all(|action| {
                matches!(action,
                    TerminalAffineCleanupAction::DiscardRoot(place) if !affine.contains(place)
                )
            }) =>
            {
                Vec::new()
            }
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards
                .iter()
                .all(|place| !affine.contains(place)) =>
            {
                Vec::new()
            }
            Terminator::Crash { .. } => Vec::new(),
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
    let mut remaining = incoming.iter().map(Vec::len).collect::<Vec<_>>();
    let mut ready = VecDeque::from([entry_position]);
    let mut entries = vec![Vec::new(); blocks.len()];
    let mut exits = vec![Vec::new(); blocks.len()];
    let mut visited = 0usize;
    while let Some(position) = ready.pop_front() {
        visited += 1;
        let mut live = if position == entry_position {
            affine.clone()
        } else {
            incoming[position]
                .first()
                .map(|predecessor| exits[*predecessor].clone())
                .unwrap_or_default()
        };
        for predecessor in &incoming[position] {
            live.retain(|place| exits[*predecessor].contains(place));
        }
        entries[position] = live.clone();
        for operation in &blocks[position].operations {
            let arguments = call_structural_arguments(&operation.kind);
            for argument in arguments.iter().filter(|argument| {
                argument.access == StructuralAccess::Owned && managed.contains(&argument.place)
            }) {
                if !argument.path.is_empty() {
                    return unsupported(
                        "scalar graph affine parameter transfer requires its whole place",
                    );
                }
                let Some(live_position) = live.iter().position(|place| *place == argument.place)
                else {
                    return unsupported(
                        "scalar graph owned call transfers an unavailable affine parameter",
                    );
                };
                live.remove(live_position);
            }
            if let Some(result) = operation.result.structural()
                && births.iter().any(|(place, _)| *place == result.place)
            {
                if live.contains(&result.place) {
                    return unsupported("scalar graph repeats a live descriptor birth");
                }
                live.insert(0, result.place);
            }
        }
        exits[position] = live;
        for successor in &successors[position] {
            if reentered && *successor == entry_position {
                continue;
            }
            remaining[*successor] -= 1;
            if remaining[*successor] == 0 {
                ready.push_back(*successor);
            }
        }
    }
    if visited != blocks.len() {
        return unsupported(
            "owned scalar graph has unreachable or cyclic custody requiring structural forwarding evidence",
        );
    }
    // A join keeps precisely the owners live on every incoming path. The
    // other paths dispose their surviving roots on their own selected edge.
    // A later read cannot revive a discarded owner; frontier verification
    // checks every operation against the reconciled entry.
    let producers = blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            operation
                .result
                .structural()
                .map(|result| (result.place, operation.id))
        })
        .collect::<Vec<_>>();
    for (position, block) in blocks.iter_mut().enumerate() {
        let discards = |target_position: usize| {
            exits[position]
                .iter()
                .copied()
                .filter(|place| !entries[target_position].contains(place))
                .collect::<Vec<_>>()
        };
        match &mut block.terminator {
            Terminator::Jump {
                target,
                structural_arguments,
                trivial_affine_discards,
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
                    merge_discards(trivial_affine_discards, remaining, &producers);
                } else {
                    merge_discards(
                        trivial_affine_discards,
                        discards(successors[position][0]),
                        &producers,
                    );
                }
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                merge_discards(
                    &mut when_true.trivial_affine_discards,
                    discards(successors[position][0]),
                    &producers,
                );
                merge_discards(
                    &mut when_false.trivial_affine_discards,
                    discards(successors[position][1]),
                    &producers,
                );
            }
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => {
                merge_discards(trivial_affine_discards, exits[position].clone(), &producers);
            }
            Terminator::Return {
                cleanup_actions, ..
            } => {
                let mut places = cleanup_actions
                    .iter()
                    .map(|action| match action {
                        TerminalAffineCleanupAction::DiscardRoot(place) => *place,
                        _ => unreachable!("cleanup shape checked above"),
                    })
                    .collect();
                merge_discards(&mut places, exits[position].clone(), &producers);
                *cleanup_actions = places
                    .into_iter()
                    .map(TerminalAffineCleanupAction::DiscardRoot)
                    .collect();
            }
            _ => {}
        }
    }
    Ok(())
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
    existing: &mut Vec<semantic_vocabulary::PlaceId>,
    additions: Vec<semantic_vocabulary::PlaceId>,
    producers: &[(
        semantic_vocabulary::PlaceId,
        semantic_vocabulary::OperationId,
    )],
) {
    let producer = |place| {
        producers
            .iter()
            .find_map(|(candidate, operation)| (*candidate == place).then_some(*operation))
    };
    for place in additions {
        if let Some(birth) = producer(place) {
            let position = existing
                .iter()
                .position(|current| {
                    producer(*current).is_none_or(|current_birth| current_birth < birth)
                })
                .unwrap_or(existing.len());
            existing.insert(position, place);
        } else {
            existing.push(place);
        }
    }
}

fn scheduled_discards(terminator: &Terminator) -> Vec<semantic_vocabulary::PlaceId> {
    match terminator {
        Terminator::Jump {
            trivial_affine_discards,
            ..
        }
        | Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        }
        | Terminator::ReturnUnitPartialAffine {
            trivial_affine_discards,
            ..
        }
        | Terminator::ReturnStructural {
            trivial_affine_discards,
            ..
        } => trivial_affine_discards.clone(),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => when_true
            .trivial_affine_discards
            .iter()
            .chain(&when_false.trivial_affine_discards)
            .copied()
            .collect(),
        Terminator::Return {
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
            .flat_map(|edge| edge.trivial_affine_discards.iter().copied())
            .collect(),
        _ => Vec::new(),
    }
}
