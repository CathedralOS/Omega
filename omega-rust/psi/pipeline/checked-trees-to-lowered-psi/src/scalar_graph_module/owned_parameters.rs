//! Complete whole, claim-free affine parameter disposal in scalar graphs.

use super::*;
use std::collections::VecDeque;

#[cfg(test)]
mod tests;

/// The caller must first establish source no-code signature eligibility through
/// parameter_storage -> owned::validate, including permissions and exclusion of
/// nominal cleanup. Terminal multiplicity alone does not establish eligibility.
/// This private pass completes that custody through calls and continuations;
/// the independent verifier checks types, reads, and disposal completeness.
pub(super) fn complete(
    parameters: &[StructuralParameterDeclaration],
    entry: BlockId,
    blocks: &mut [Block],
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
    if affine.is_empty() {
        return Ok(());
    }
    if affine
        .iter()
        .enumerate()
        .any(|(position, place)| affine[..position].contains(place))
    {
        return unsupported("scalar graph affine parameters have duplicate places");
    }
    let entry_position = block_position(blocks, entry)?;
    let reentered = !blocks[entry_position].structural_parameters.is_empty();
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
                residual_affine_discards,
                trivial_affine_discards,
                ..
            } => {
                if (structural_arguments
                    .iter()
                    .any(|argument| affine.contains(&argument.place))
                    && !(reentered && *target == entry))
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
                    edge.structural_arguments
                        .iter()
                        .any(|argument| affine.contains(&argument.place))
                        || edge
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
            Terminator::Crash { .. } => Vec::new(),
            _ => {
                return unsupported(
                    "scalar graph whole parameter cleanup requires scalar exits and ordinary edges",
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
        let mut live = affine.clone();
        for predecessor in &incoming[position] {
            live.retain(|place| exits[*predecessor].contains(place));
        }
        entries[position] = live.clone();
        for operation in &blocks[position].operations {
            let arguments = match &operation.kind {
                OperationKind::CallUnit {
                    structural_arguments,
                    ..
                }
                | OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                }
                | OperationKind::CallStructural {
                    structural_arguments,
                    ..
                }
                | OperationKind::CallStructuralWithScalarArguments {
                    structural_arguments,
                    ..
                }
                | OperationKind::BoundaryCall {
                    structural_arguments,
                    ..
                } => structural_arguments.as_slice(),
                _ => &[],
            };
            for argument in arguments.iter().filter(|argument| {
                argument.access == StructuralAccess::Owned && affine.contains(&argument.place)
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
                    trivial_affine_discards.extend(remaining);
                } else {
                    trivial_affine_discards.extend(discards(successors[position][0]));
                }
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                when_true
                    .trivial_affine_discards
                    .extend(discards(successors[position][0]));
                when_false
                    .trivial_affine_discards
                    .extend(discards(successors[position][1]));
            }
            Terminator::Return {
                cleanup_actions, ..
            } => {
                cleanup_actions.extend(
                    exits[position]
                        .iter()
                        .copied()
                        .map(TerminalAffineCleanupAction::DiscardRoot),
                );
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
