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
//! A separately selected whole-home discard can enter the same equations only
//! to withdraw its receipt after a move. Tracking that home never extends the
//! source-checked disposal capability roster or schedules another death.
//! Cycles preserve the same finite carrier frontier; nominal cleanup remains
//! outside this pass.
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::{BlockId, StructuralPlaceKind};
use terminal_psi::{
    Block, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, TerminalAffineCleanupAction, Terminator,
};

mod carrier_arrivals;
mod custody_flow;
mod projected_transfers;

pub(crate) use projected_transfers::{
    copied_operand_roots, operands as structural_operands, owned_extractions, partitioned_roots,
    transfers_custody,
};

/// Join source-authorized no-code type capability to this closure's existing
/// normalized type identities. Terminal storage geometry alone cannot authorize
/// disposal, and a final death row cannot describe a skipped earlier transfer.
pub(crate) fn no_code_type_ids(
    checked: &checked_trees::CheckedTrees,
    type_ids: &[(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
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
/// authentic homes for move tracking and residual geometry. Only the checked
/// capability roster authorizes additional owned local/result and
/// receiving-block homes. Actual affine access,
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
    let extractions =
        projected_transfers::owned_extractions(entry_parameters, local_places, blocks, types)?;
    complete_carriers(
        parameters,
        entry_parameters,
        no_code_types,
        local_places,
        entry,
        blocks,
        types,
        &extractions,
    )?;
    projected_transfers::complete(
        entry_parameters,
        local_places,
        entry,
        blocks,
        types,
        &extractions,
    )
}

fn complete_carriers(
    parameters: &[StructuralParameterDeclaration],
    entry_parameters: &[StructuralParameterDeclaration],
    no_code_types: &[semantic_vocabulary::StructuralTypeId],
    local_places: &[StructuralPlaceDeclaration],
    entry: BlockId,
    blocks: &mut [Block],
    types: &[terminal_psi::StructuralTypeDeclaration],
    extractions: &[(
        semantic_vocabulary::OperationId,
        terminal_psi::StructuralArgument,
    )],
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
    let copied_roots = copied_operand_roots(entry_parameters, blocks);
    let partial_transfers = partitioned_roots(blocks, extractions, &copied_roots)
        .into_iter()
        .collect::<Vec<_>>();
    let mut births = blocks
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
            // Membership metadata neither settles this carrier's disposal
            // debt nor authorizes disposal. Keep it on the result; the exact
            // checked type or anchored loan-carrier authority below decides
            // whether this pass may settle its claim-free affine custody.
            if result.multiplicity != StructuralMultiplicity::Affine || !result.claims.is_empty() {
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
                parameter.access == StructuralAccess::Owned
                    && parameter.multiplicity == StructuralMultiplicity::Affine
                    && !affine.contains(&parameter.place)
                    && no_code_types.contains(&parameter.structural_type)
                    && !partial_transfers.contains(&parameter.place)
            })
            .map(|parameter| parameter.place),
    );
    carrier_arrivals::inherit(
        &mut managed,
        parameters,
        local_places,
        blocks,
        types,
        &partial_transfers,
        &scheduled,
    )?;
    // A selected whole death is already source-authorized. Track its actual
    // home in the same frontier equations so a completed move can withdraw
    // that receipt. This is not permission to schedule another death: these
    // homes remain outside the disposal capability roster.
    let disposal_capability = managed.clone();
    let mut tracked_disposals = scheduled
        .iter()
        .copied()
        .filter(|place| {
            !managed.contains(place)
                && !partial_transfers.contains(place)
                && carrier_arrivals::owned_affine_type(
                    entry_parameters,
                    local_places,
                    blocks,
                    *place,
                )
                .is_some()
                && blocks
                    .iter()
                    .flat_map(|block| scheduled_actions(&block.terminator))
                    .filter(|action| action.place() == *place)
                    .all(|action| matches!(action, TerminalAffineCleanupAction::DiscardRoot(_)))
        })
        .collect::<Vec<_>>();
    tracked_disposals.sort_unstable();
    tracked_disposals.dedup();
    let initial_owners = affine
        .iter()
        .copied()
        .chain(
            entry_parameters
                .iter()
                .filter(|parameter| tracked_disposals.contains(&parameter.place))
                .map(|parameter| parameter.place),
        )
        .collect::<Vec<_>>();
    births.extend(
        blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| {
                let place = established_place(operation)?;
                tracked_disposals
                    .contains(&place)
                    .then_some((place, operation.id))
            }),
    );
    managed.extend(&tracked_disposals);
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
        &initial_owners,
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
    let complete_disposals = |actions: &mut Vec<TerminalAffineCleanupAction>,
                              additions: Vec<semantic_vocabulary::PlaceId>,
                              live: &[semantic_vocabulary::PlaceId]| {
        actions.retain(|action| {
            !matches!(action,
                TerminalAffineCleanupAction::DiscardRoot(place)
                    if tracked_disposals.contains(place) && !live.contains(place)
            )
        });
        replace_managed_discards(
            actions,
            additions
                .into_iter()
                .filter(|place| disposal_capability.contains(place))
                .collect(),
            &disposal_capability,
            &producers,
            &parameter_order,
        );
    };
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
                    complete_disposals(cleanup_actions, remaining, &outgoing[position][0]);
                } else {
                    complete_disposals(
                        cleanup_actions,
                        discards(0, successors[position][0]),
                        &outgoing[position][0],
                    );
                }
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                complete_disposals(
                    &mut when_true.cleanup_actions,
                    discards(0, successors[position][0]),
                    &outgoing[position][0],
                );
                complete_disposals(
                    &mut when_false.cleanup_actions,
                    discards(1, successors[position][1]),
                    &outgoing[position][1],
                );
            }
            Terminator::StructuralCase { cases, .. } => {
                for (edge_position, (edge, successor)) in
                    cases.iter_mut().zip(&successors[position]).enumerate()
                {
                    complete_disposals(
                        &mut edge.cleanup_actions,
                        discards(edge_position, *successor),
                        &outgoing[position][edge_position],
                    );
                }
            }
            terminal_psi::return_or_return_structural_pattern!(cleanup_actions, ..)
            | Terminator::ReturnUnit {
                cleanup_actions, ..
            } => {
                complete_disposals(cleanup_actions, exits[position].clone(), &exits[position]);
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
    let mut additions = additions
        .into_iter()
        .filter(|place| {
            !existing.iter().any(|action| {
                matches!(action, TerminalAffineCleanupAction::RetireHome(retired) if retired == place)
            })
        })
        .collect::<Vec<_>>();
    // Recompute which managed roots still require disposal without rebuilding
    // the selected cleanup order. A descriptor can have moved into a block
    // parameter while its operation-produced backing remains live; replacing
    // both deaths in producer order would discard the backing first.
    existing.retain(|action| {
        !matches!(action, TerminalAffineCleanupAction::DiscardRoot(place)
            if managed.contains(place) && !additions.contains(place))
    });
    additions.retain(|place| {
        !existing.iter().any(|action| {
            matches!(action, TerminalAffineCleanupAction::DiscardRoot(scheduled_place) if scheduled_place == place)
        })
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
    scheduled_actions(terminator)
        .into_iter()
        .filter_map(|action| match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
            _ => None,
        })
        .collect()
}

fn scheduled_actions(terminator: &Terminator) -> Vec<&TerminalAffineCleanupAction> {
    match terminator {
        Terminator::Jump {
            cleanup_actions, ..
        } => cleanup_actions.iter().collect(),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => when_true
            .cleanup_actions
            .iter()
            .chain(&when_false.cleanup_actions)
            .collect(),
        terminal_psi::return_or_return_structural_pattern!(cleanup_actions, ..)
        | Terminator::ReturnUnit {
            cleanup_actions, ..
        } => cleanup_actions.iter().collect(),
        Terminator::StructuralCase { cases, .. } => cases
            .iter()
            .flat_map(|edge| &edge.cleanup_actions)
            .collect(),
        _ => Vec::new(),
    }
}
