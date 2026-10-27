//! Complete no-code affine carrier custody after a body's operation graph exists.
//!
//! Callers supply source-checked no-code disposal capability independently of
//! any scheduled death. The actual operation graph supplies births and selected
//! transfers; their finite frontier equations settle every normal predecessor
//! before a join. View and reference carriers retain their separate loan cleanup
//! authority and die before their backing owners.
//! Carrier disposal grants no ownership of borrowed backing storage. Borrowing
//! a carrier across an edge retains its original disposal debt until exit.
//! An affine borrowed view that arrives as a parameter owes that same carrier
//! death. When no loan action names it, every normal return it can reach
//! discards the descriptor and leaves the borrowed elements in place.
//! Region disposal retains its exact checked action stream independently of
//! whole-home liveness; explicit retirement ends that home's current epoch.
//! A separately selected whole-home discard can enter the same equations only
//! to withdraw its receipt after a move. Tracking that home never extends the
//! source-checked disposal capability roster or schedules another death.
//! Cycles preserve the same finite carrier frontier; nominal cleanup remains
//! outside this pass.
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind};
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
    let no_code: std::collections::HashSet<typed_trees::type_identity::TypeIdentityId> = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter()
        .filter(|declaration| declaration.no_code_affine_cleanup)
        .map(|declaration| declaration.identity)
        .collect();
    type_ids
        .iter()
        .filter_map(|(identity, id)| no_code.contains(identity).then_some(*id))
        .collect()
}

/// Complete source-checked no-code custody in the emitted operation graph.
/// Copyable descriptor aggregates retain their source on ordinary copies and
/// end only the physical binding with RetireHome; affine carriers dispose their
/// remaining value. Both use the same actual birth and selected-edge equations.
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
    let _g0 = phase_profile::guard("05.01.g1.3e.1_owned_extractions");
    let extractions =
        projected_transfers::owned_extractions(entry_parameters, local_places, blocks, types)?;
    drop(_g0);
    let _g1 = phase_profile::guard("05.01.g1.3e.2_complete_carriers");
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
    drop(_g1);
    let _g2 = phase_profile::guard("05.01.g1.3e.3_projected_transfers");
    projected_transfers::complete(
        entry_parameters,
        local_places,
        entry,
        blocks,
        types,
        &extractions,
    )?;
    drop(_g2);
    let _g3 = phase_profile::guard("05.01.g1.3e.4_retire_borrowed");
    retire_borrowed_descriptor_arrivals(entry_parameters, entry, blocks, types)
}

/// Close affine borrowed view arrivals that no selected loan death names.
///
/// A mutable byte view can be copied into a block parameter. That parameter
/// is an affine descriptor: execution records the carrier on the live
/// frontier at the arrival and accepts a normal return only after the
/// carrier is discarded. The discard does not own the borrowed elements.
/// Loan cleanup, an owned transfer, or a reference release already accounts
/// for the arrivals it names. A structural return of the same place keeps
/// the frontier entry that result transfer checks.
fn retire_borrowed_descriptor_arrivals(
    entry_parameters: &[StructuralParameterDeclaration],
    entry: BlockId,
    blocks: &mut [Block],
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    let mut arrivals = Vec::new();
    for parameter in entry_parameters
        .iter()
        .filter(|parameter| borrowed_descriptor_arrival(parameter, types))
    {
        if !arrivals.iter().any(|(place, _)| *place == parameter.place) {
            arrivals.push((parameter.place, vec![entry]));
        }
    }
    for block in blocks.iter() {
        for parameter in block
            .structural_parameters
            .iter()
            .filter(|parameter| borrowed_descriptor_arrival(parameter, types))
        {
            if let Some((_, births)) = arrivals
                .iter_mut()
                .find(|(place, _)| *place == parameter.place)
            {
                if !births.contains(&block.id) {
                    births.push(block.id);
                }
            } else {
                arrivals.push((parameter.place, vec![block.id]));
            }
        }
    }
    if !arrivals.is_empty() {
        let mut scheduled_deaths: std::collections::HashSet<PlaceId> =
            std::collections::HashSet::new();
        for block in blocks.iter() {
            for (_, actions) in block.terminator.cleanup_rosters() {
                for action in actions {
                    scheduled_deaths.insert(action.place());
                }
            }
            for operation in &block.operations {
                for argument in structural_operands(&operation.kind) {
                    if argument.path.is_empty() && argument.access == StructuralAccess::Owned {
                        scheduled_deaths.insert(argument.place);
                    }
                }
                if let terminal_psi::OperationKind::ReleaseReference { source, path } =
                    &operation.kind
                    && path.is_empty()
                {
                    scheduled_deaths.insert(*source);
                }
            }
            let record_owned = |argument: &terminal_psi::StructuralArgument,
                                set: &mut std::collections::HashSet<PlaceId>| {
                if argument.path.is_empty() && argument.access == StructuralAccess::Owned {
                    set.insert(argument.place);
                }
            };
            match &block.terminator {
                Terminator::Jump {
                    structural_arguments,
                    ..
                }
                | Terminator::ExternalCompletion {
                    structural_arguments,
                    ..
                } => {
                    for arg in structural_arguments {
                        record_owned(arg, &mut scheduled_deaths);
                    }
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for arg in when_true
                        .structural_arguments
                        .iter()
                        .chain(&when_false.structural_arguments)
                    {
                        record_owned(arg, &mut scheduled_deaths);
                    }
                }
                _ => {}
            }
        }
        let pending = arrivals
            .into_iter()
            .filter(|(place, _)| !scheduled_deaths.contains(place))
            .collect::<Vec<_>>();
        for (place, births) in pending {
            let mut reachable_set: std::collections::HashSet<BlockId> =
                births.iter().copied().collect();
            let mut reachable = births;
            let mut cursor = 0;
            while cursor < reachable.len() {
                let current = reachable[cursor];
                cursor += 1;
                let Some(block_index) = blocks.binary_search_by_key(&current, |block| block.id).ok()
                else {
                    continue;
                };
                for target in blocks[block_index].terminator.successor_targets() {
                    if reachable_set.insert(target) {
                        reachable.push(target);
                    }
                }
            }
            for block in blocks.iter_mut() {
                if !reachable_set.contains(&block.id) {
                    continue;
                }
                if matches!(
                    block.terminator,
                    Terminator::ReturnStructural { source, .. } if source == place
                ) {
                    continue;
                }
                let Some(actions) = return_cleanup(&mut block.terminator) else {
                    continue;
                };
                if actions.iter().any(|action| action.place() == place) {
                    continue;
                }
                actions.push(TerminalAffineCleanupAction::DiscardRoot(place));
            }
        }
    }
    Ok(())
}

fn borrowed_descriptor_arrival(
    parameter: &StructuralParameterDeclaration,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    parameter.multiplicity == StructuralMultiplicity::Affine
        && parameter.access != StructuralAccess::Owned
        && terminal_semantics::view_descriptor_parameter(parameter, types.iter())
}

fn return_cleanup(terminator: &mut Terminator) -> Option<&mut Vec<TerminalAffineCleanupAction>> {
    match terminator {
        Terminator::Return {
            cleanup_actions, ..
        }
        | Terminator::ReturnUnit {
            cleanup_actions, ..
        }
        | Terminator::ReturnStructural {
            cleanup_actions, ..
        } => Some(cleanup_actions),
        _ => None,
    }
}

pub(super) fn copyable_descriptor_type_ids(
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Vec<semantic_vocabulary::StructuralTypeId> {
    // Cleanup compatibility permits plain storage too; it does not establish
    // that storage contains a descriptor. Only actual borrowed descriptor
    // payloads need this additional home lifetime. Plain unrestricted payloads
    // retain their ordinary producer/storage lifetime without disposal debt.
    // Retirement ends only a descriptor binding, never its referent. Direct
    // reference/view shells keep their existing loan-continuation owner.
    types
        .iter()
        .filter(|declaration| {
            declaration.descriptor_cleanup
                && terminal_psi::structural_type_has_borrowed_storage_by(
                    |identity| types.iter().find(|candidate| candidate.id == identity),
                    declaration.id,
                )
                && terminal_semantics::discardable_store_value(
                    types.iter(),
                    declaration.id,
                    StructuralMultiplicity::Unrestricted,
                    false,
                )
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::Record { .. }
                        | terminal_psi::StructuralTypeShape::FixedArray { .. }
                        | terminal_psi::StructuralTypeShape::Sum { .. }
                        | terminal_psi::StructuralTypeShape::Mixed { .. }
                )
        })
        .map(|declaration| declaration.id)
        .collect()
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
    let copyable_descriptor_types = copyable_descriptor_type_ids(types);
    let copyable_descriptor_set: std::collections::HashSet<semantic_vocabulary::StructuralTypeId> =
        copyable_descriptor_types.iter().copied().collect();
    let no_code_type_set: std::collections::HashSet<semantic_vocabulary::StructuralTypeId> =
        no_code_types.iter().copied().collect();
    let entry_owners = parameters
        .iter()
        .rev()
        .filter_map(|parameter| {
            (parameter.access == StructuralAccess::Owned
                && (parameter.multiplicity == StructuralMultiplicity::Affine
                    || (parameter.multiplicity == StructuralMultiplicity::Unrestricted
                        && copyable_descriptor_set.contains(&parameter.structural_type))))
            .then_some(parameter.place)
        })
        .collect::<Vec<_>>();
    // Descriptor cleanup remains owned by its loan continuation. Plain owned
    // type capability is independent of this already scheduled death roster.
    let scheduled = blocks
        .iter()
        .flat_map(|block| scheduled_discards(&block.terminator))
        .collect::<Vec<_>>();
    let scheduled_set: std::collections::HashSet<PlaceId> = scheduled.iter().copied().collect();
    // A partial transfer retains root storage and its independently selected
    // residual disposal. The whole-home solver must not replace that schedule.
    let copied_roots = copied_operand_roots(entry_parameters, blocks);
    let partial_transfers_set = partitioned_roots(blocks, extractions, &copied_roots);
    let partial_transfers = partial_transfers_set.iter().copied().collect::<Vec<_>>();
    let mut unique_type_by_id: std::collections::HashMap<
        semantic_vocabulary::StructuralTypeId,
        Option<&terminal_psi::StructuralTypeDeclaration>,
    > = std::collections::HashMap::with_capacity(types.len());
    for declaration in types {
        match unique_type_by_id.entry(declaration.id) {
            std::collections::hash_map::Entry::Vacant(vacant) => {
                vacant.insert(Some(declaration));
            }
            std::collections::hash_map::Entry::Occupied(mut occupied) => {
                *occupied.get_mut() = None;
            }
        }
    }
    let trivial_affine_locals: std::collections::HashSet<PlaceId> = local_places
        .iter()
        .filter_map(|local| match local.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } if no_code_type_set.contains(&structural_type) => Some(local.id),
            _ => None,
        })
        .collect();
    let mut births = blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            let place = established_place(operation)?;
            if partial_transfers_set.contains(&place) {
                return None;
            }
            let Some(result) = operation.result.structural() else {
                // In-place establishment owns a declared local home, not an
                // operation result. Rejoin its exact checked type capability.
                return trivial_affine_locals
                    .contains(&place)
                    .then_some((place, operation.id));
            };
            // Membership metadata neither settles this carrier's disposal
            // debt nor authorizes disposal. Keep it on the result; the exact
            // checked type or anchored loan-carrier authority below decides
            // whether this pass may settle its claim-free carrier custody.
            let copyable_home = result.multiplicity == StructuralMultiplicity::Unrestricted
                && copyable_descriptor_set.contains(&result.structural_type);
            if (result.multiplicity != StructuralMultiplicity::Affine && !copyable_home)
                || !result.claims.is_empty()
            {
                return None;
            }
            let declaration = unique_type_by_id
                .get(&result.structural_type)
                .copied()
                .flatten()?;
            let unscheduled_carrier = !scheduled_set.contains(&result.place)
                && (matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ElementView { .. }
                        | terminal_psi::StructuralTypeShape::Reference { .. }
                ) || declaration.shape.is_borrowed_byte_view());
            if !(no_code_type_set.contains(&result.structural_type)
                || unscheduled_carrier
                || copyable_home)
            {
                return None;
            }
            Some((result.place, operation.id))
        })
        .collect::<Vec<_>>();
    let mut managed = entry_owners.clone();
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
                    && (parameter.multiplicity == StructuralMultiplicity::Affine
                        && no_code_type_set.contains(&parameter.structural_type)
                        || parameter.multiplicity == StructuralMultiplicity::Unrestricted
                            && copyable_descriptor_set.contains(&parameter.structural_type))
                    && !entry_owners.contains(&parameter.place)
                    && !partial_transfers_set.contains(&parameter.place)
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
    let disposal_capability_set: std::collections::HashSet<PlaceId> =
        disposal_capability.iter().copied().collect();
    let mut tracked_disposals = if scheduled.is_empty() {
        Vec::new()
    } else {
        let owned_affine_types =
            carrier_arrivals::build_owned_affine_types(entry_parameters, local_places, blocks);
        let mut non_discard_root_places: std::collections::HashSet<PlaceId> =
            std::collections::HashSet::new();
        for block in blocks.iter() {
            for action in scheduled_actions(&block.terminator) {
                if !matches!(action, TerminalAffineCleanupAction::DiscardRoot(_)) {
                    non_discard_root_places.insert(action.place());
                }
            }
        }
        scheduled
            .iter()
            .copied()
            .filter(|place| {
                !disposal_capability_set.contains(place)
                    && !partial_transfers_set.contains(place)
                    && owned_affine_types.get(place).copied().flatten().is_some()
                    && !non_discard_root_places.contains(place)
            })
            .collect::<Vec<_>>()
    };
    tracked_disposals.sort_unstable();
    tracked_disposals.dedup();
    let tracked_disposals_set: std::collections::HashSet<PlaceId> =
        tracked_disposals.iter().copied().collect();
    let initial_owners = entry_owners
        .iter()
        .copied()
        .chain(
            entry_parameters
                .iter()
                .filter(|parameter| tracked_disposals_set.contains(&parameter.place))
                .map(|parameter| parameter.place),
        )
        .collect::<Vec<_>>();
    births.extend(
        blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| {
                let place = established_place(operation)?;
                tracked_disposals_set
                    .contains(&place)
                    .then_some((place, operation.id))
            }),
    );
    managed.extend(&tracked_disposals);
    if managed.is_empty() {
        return Ok(());
    }
    let mut managed_set: std::collections::HashSet<PlaceId> =
        std::collections::HashSet::with_capacity(managed.len());
    for &place in &managed {
        if !managed_set.insert(place) {
            return unsupported("scalar graph affine parameters have duplicate places");
        }
    }
    let entry_position = block_position(blocks, entry)?;
    let reentered =
        !entry_owners.is_empty() && !blocks[entry_position].structural_parameters.is_empty();
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
                .any(|parameter| entry_owners.contains(&parameter.place))
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
                    entry_owners.contains(&argument.place)
                        && projected_transfers::transfers_custody(argument, &copied_roots)
                }) && !(reentered && *target == entry))
                    || cleanup_actions
                        .iter()
                        .any(|action| competing_cleanup(action, &entry_owners, &managed_set))
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
                        entry_owners.contains(&argument.place)
                            && projected_transfers::transfers_custody(argument, &copied_roots)
                    }) || edge
                        .cleanup_actions
                        .iter()
                        .any(|action| competing_cleanup(action, &entry_owners, &managed_set))
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
                        .any(|action| competing_cleanup(action, &entry_owners, &managed_set))
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
                .all(|action| !competing_cleanup(action, &entry_owners, &managed_set)) =>
            {
                Vec::new()
            }
            // An ordinary nonreturning invocation retains the caller's owners
            // and loans. It has no local successor and grants no disposal.
            Terminator::InvokeNoNormal { .. } => Vec::new(),
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
        &copied_roots,
        &incoming,
        &successors,
    )?;
    // A join keeps precisely the owners live on every incoming path. The
    // other paths dispose their surviving roots on their own selected edge.
    // A later read cannot revive a discarded owner; frontier verification
    // checks every operation against the reconciled entry.
    let mut producers: std::collections::HashMap<
        semantic_vocabulary::PlaceId,
        semantic_vocabulary::OperationId,
    > = std::collections::HashMap::new();
    for operation in blocks.iter().flat_map(|block| &block.operations) {
        if let Some(place) = established_place(operation) {
            producers.entry(place).or_insert(operation.id);
        }
    }
    let dominators =
        crate::emission::control_flow::block_dominators(Some(entry_position), &successors);
    let mut parameter_blocks = blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| !block.structural_parameters.is_empty())
        .collect::<Vec<_>>();
    parameter_blocks.sort_by_key(|(position, _)| dominators[*position].len());
    let mut parameter_order: std::collections::HashMap<semantic_vocabulary::PlaceId, usize> =
        std::collections::HashMap::new();
    for (index, place) in parameters
        .iter()
        .map(|parameter| parameter.place)
        .chain(
            parameter_blocks
                .iter()
                .flat_map(|(_, block)| &block.structural_parameters)
                .map(|parameter| parameter.place),
        )
        .enumerate()
    {
        parameter_order.insert(place, index);
    }
    let complete_disposals = |actions: &mut Vec<TerminalAffineCleanupAction>,
                              additions: Vec<semantic_vocabulary::PlaceId>,
                              live: &[semantic_vocabulary::PlaceId]| {
        actions.retain(|action| {
            !matches!(action,
                TerminalAffineCleanupAction::DiscardRoot(place)
                    if tracked_disposals_set.contains(place) && !live.contains(place)
            )
        });
        replace_managed_discards(
            actions,
            additions
                .into_iter()
                .filter(|place| disposal_capability_set.contains(place))
                .collect(),
            &disposal_capability_set,
            &producers,
            &parameter_order,
            &copied_roots,
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
    managed: &std::collections::HashSet<semantic_vocabulary::PlaceId>,
    producers: &std::collections::HashMap<
        semantic_vocabulary::PlaceId,
        semantic_vocabulary::OperationId,
    >,
    parameter_order: &std::collections::HashMap<semantic_vocabulary::PlaceId, usize>,
    copied_roots: &std::collections::BTreeSet<semantic_vocabulary::PlaceId>,
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
    merge_discards(
        existing,
        additions,
        producers,
        parameter_order,
        copied_roots,
    );
}

/// Regional cleanup may coexist with a whole-home lifetime equation. Nominal
/// cleanup retains its independent ordered owner; delegated entry root disposal
/// must still be supplied exactly once by this invocation.
fn competing_cleanup(
    action: &TerminalAffineCleanupAction,
    affine: &[semantic_vocabulary::PlaceId],
    managed: &std::collections::HashSet<semantic_vocabulary::PlaceId>,
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
    if let Ok(index) = blocks.binary_search_by_key(&target, |block| block.id) {
        let duplicate_before = index > 0 && blocks[index - 1].id == target;
        let duplicate_after = index + 1 < blocks.len() && blocks[index + 1].id == target;
        if !duplicate_before && !duplicate_after {
            return Ok(index);
        }
    }
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
    producers: &std::collections::HashMap<
        semantic_vocabulary::PlaceId,
        semantic_vocabulary::OperationId,
    >,
    parameter_order: &std::collections::HashMap<semantic_vocabulary::PlaceId, usize>,
    copied_roots: &std::collections::BTreeSet<semantic_vocabulary::PlaceId>,
) {
    for place in additions {
        let insertion = disposal_position(
            existing.iter().map(TerminalAffineCleanupAction::place),
            place,
            producers,
            parameter_order,
        );
        let action = if copied_roots.contains(&place) {
            TerminalAffineCleanupAction::RetireHome(place)
        } else {
            TerminalAffineCleanupAction::DiscardRoot(place)
        };
        existing.insert(insertion, action);
    }
}

fn disposal_position(
    existing: impl Iterator<Item = semantic_vocabulary::PlaceId>,
    place: semantic_vocabulary::PlaceId,
    producers: &std::collections::HashMap<
        semantic_vocabulary::PlaceId,
        semantic_vocabulary::OperationId,
    >,
    parameter_order: &std::collections::HashMap<semantic_vocabulary::PlaceId, usize>,
) -> usize {
    let birth = producers.get(&place).copied();
    let parameter_position = parameter_order.get(&place).copied();
    let mut length = 0;
    for (position, current) in existing.enumerate() {
        if birth.is_some_and(|birth| {
            producers
                .get(&current)
                .is_none_or(|&current_birth| current_birth < birth)
        }) || (birth.is_none()
            && parameter_position.is_some_and(|parameter_position| {
                parameter_order
                    .get(&current)
                    .is_some_and(|&current_position| current_position < parameter_position)
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
