//! How each terminator kind closes a block and hands its frontier to the
//! block's successors.

use super::super::{
    BTreeMap, BlockId, ModuleError, OperationKind, StructuralAccess, StructuralMultiplicity,
    StructuralPlaceKind, Terminator, partial_affine_residuals, partial_affine_root_type,
};
use super::{
    FrontierWalk, StructuralOwnershipFrontier, VerifiedMachineStructuralFrontiers,
    apply_continuation_residual_discards, apply_edge_trivial_affine_discards,
    expected_trivial_affine_discards, release_reference_discards, require_no_references,
    validate_scalar_cleanup_actions,
};

/// Closes a block: every successful exit closes its remaining loans, then
/// the terminator kind hands the frontier to its successors or checks the
/// machine's exit custody.
pub(super) fn close_block(
    walk: &FrontierWalk<'_>,
    block: &terminal_psi::Block,
    frontier: StructuralOwnershipFrontier,
    snapshots: &mut VerifiedMachineStructuralFrontiers,
    incoming: &mut BTreeMap<BlockId, Vec<StructuralOwnershipFrontier>>,
) -> Result<(), ModuleError> {
    let FrontierWalk {
        module, machine, ..
    } = *walk;
    // Every successful exit closes its remaining loans. The ordinary
    // ownership checks below independently validate the complete roster.
    let closing = match &block.terminator {
        Terminator::ReturnUnitPartialAffine {
            trivial_affine_discards,
            ..
        } => Some(trivial_affine_discards.clone()),
        Terminator::ReturnUnitNominalAffine { .. } => Some(Vec::new()),
        _ => None,
    };
    if let Some(discards) = closing {
        let mut remaining = frontier.references.clone();
        release_reference_discards(module, machine, &mut remaining, &discards)?;
        require_no_references(machine, &remaining)?;
    }
    match &block.terminator {
        Terminator::Jump { .. } => close_jump(walk, block, frontier, snapshots, incoming),
        Terminator::Conditional { .. } => {
            close_conditional(walk, block, frontier, snapshots, incoming)
        }
        Terminator::StructuralCase { .. } => {
            close_structural_case(walk, block, frontier, snapshots, incoming)
        }
        Terminator::ReturnUnit { .. } => close_return_unit(walk, block, frontier),
        Terminator::ReturnUnitPartialAffine { .. } => {
            close_return_unit_partial_affine(walk, block, frontier)
        }
        Terminator::ReturnUnitNominalAffine { .. } => {
            close_return_unit_nominal_affine(walk, block, frontier)
        }
        Terminator::Return { .. } => close_return(walk, block, frontier),
        Terminator::ReturnStructural { .. } => close_return_structural(walk, block, frontier),
        Terminator::Crash { .. } => close_crash(block, frontier),
    }
}

/// Closes a block whose terminator is `Jump`.
fn close_jump(
    walk: &FrontierWalk<'_>,
    block: &terminal_psi::Block,
    mut frontier: StructuralOwnershipFrontier,
    snapshots: &mut VerifiedMachineStructuralFrontiers,
    incoming: &mut BTreeMap<BlockId, Vec<StructuralOwnershipFrontier>>,
) -> Result<(), ModuleError> {
    let FrontierWalk {
        module,
        machine,
        blocks,
        parameter_order,
        ..
    } = *walk;
    let Terminator::Jump {
        edge,
        target,
        structural_arguments,
        trivial_affine_discards,
        residual_affine_discards,
        ..
    } = &block.terminator
    else {
        unreachable!("dispatched close_jump")
    };
    // Projected successor arguments open partial custody on their
    // root before this edge's residual evidence closes it.
    super::block_parameters::consume(
        module,
        machine,
        &mut frontier,
        *edge,
        blocks[target],
        structural_arguments,
        true,
        walk.window_aliases,
    )?;
    apply_continuation_residual_discards(
        module,
        machine,
        block.id,
        &mut frontier,
        residual_affine_discards,
    )?;
    apply_edge_trivial_affine_discards(
        module,
        machine,
        parameter_order,
        &mut frontier,
        *edge,
        trivial_affine_discards,
    )?;
    super::block_parameters::establish(
        module,
        machine,
        &mut frontier,
        *edge,
        blocks[target],
        structural_arguments,
    )?;
    snapshots.edge_exits.insert(*edge, frontier.snapshot());
    incoming.entry(*target).or_default().push(frontier);
    Ok(())
}

/// Closes a block whose terminator is `Conditional`.
fn close_conditional(
    walk: &FrontierWalk<'_>,
    block: &terminal_psi::Block,
    mut frontier: StructuralOwnershipFrontier,
    snapshots: &mut VerifiedMachineStructuralFrontiers,
    incoming: &mut BTreeMap<BlockId, Vec<StructuralOwnershipFrontier>>,
) -> Result<(), ModuleError> {
    let FrontierWalk {
        module,
        machine,
        blocks,
        parameter_order,
        ..
    } = *walk;
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &block.terminator
    else {
        unreachable!("dispatched close_conditional")
    };
    let mut true_frontier = frontier.clone();
    super::block_parameters::consume(
        module,
        machine,
        &mut true_frontier,
        when_true.edge,
        blocks[&when_true.target],
        &when_true.structural_arguments,
        false,
        walk.window_aliases,
    )?;
    apply_edge_trivial_affine_discards(
        module,
        machine,
        parameter_order,
        &mut true_frontier,
        when_true.edge,
        &when_true.trivial_affine_discards,
    )?;
    super::block_parameters::establish(
        module,
        machine,
        &mut true_frontier,
        when_true.edge,
        blocks[&when_true.target],
        &when_true.structural_arguments,
    )?;
    snapshots
        .edge_exits
        .insert(when_true.edge, true_frontier.snapshot());
    incoming
        .entry(when_true.target)
        .or_default()
        .push(true_frontier);
    super::block_parameters::consume(
        module,
        machine,
        &mut frontier,
        when_false.edge,
        blocks[&when_false.target],
        &when_false.structural_arguments,
        false,
        walk.window_aliases,
    )?;
    apply_edge_trivial_affine_discards(
        module,
        machine,
        parameter_order,
        &mut frontier,
        when_false.edge,
        &when_false.trivial_affine_discards,
    )?;
    super::block_parameters::establish(
        module,
        machine,
        &mut frontier,
        when_false.edge,
        blocks[&when_false.target],
        &when_false.structural_arguments,
    )?;
    snapshots
        .edge_exits
        .insert(when_false.edge, frontier.snapshot());
    incoming
        .entry(when_false.target)
        .or_default()
        .push(frontier);
    Ok(())
}

/// Closes a block whose terminator is `StructuralCase`.
fn close_structural_case(
    walk: &FrontierWalk<'_>,
    block: &terminal_psi::Block,
    frontier: StructuralOwnershipFrontier,
    snapshots: &mut VerifiedMachineStructuralFrontiers,
    incoming: &mut BTreeMap<BlockId, Vec<StructuralOwnershipFrontier>>,
) -> Result<(), ModuleError> {
    let FrontierWalk {
        module,
        machine,
        parameter_order,
        ..
    } = *walk;
    let Terminator::StructuralCase { source, cases } = &block.terminator else {
        unreachable!("dispatched close_structural_case")
    };
    super::super::borrowed_windows::check_case_source(
        machine,
        block.id,
        *source,
        &frontier,
        walk.window_aliases,
    )?;
    let owned_subject = machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .any(|parameter| {
            parameter.place == *source
                && parameter.access == StructuralAccess::Owned
                && parameter.multiplicity != StructuralMultiplicity::Unrestricted
        })
        || machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| {
                operation.result.structural().is_some_and(|result| {
                    result.place == *source
                        && result.multiplicity != StructuralMultiplicity::Unrestricted
                })
            });
    if owned_subject
        && (!frontier.owned_places.contains_key(source)
            || frontier.partial_custody_paths.contains_key(source))
    {
        return Err(ModuleError::StructuralCaseSourceUnknown {
            machine: machine.id,
            block: block.id,
            place: *source,
        });
    }
    for case in cases {
        let mut case_frontier = frontier.clone();
        apply_edge_trivial_affine_discards(
            module,
            machine,
            parameter_order,
            &mut case_frontier,
            case.edge,
            &case.trivial_affine_discards,
        )?;
        snapshots
            .edge_exits
            .insert(case.edge, case_frontier.snapshot());
        incoming.entry(case.target).or_default().push(case_frontier);
    }
    Ok(())
}

/// An owned `self` receiver is the language's terminal-consumer input: the
/// caller hands it into the machine and the machine consumes it by
/// completing, so claims rooted at its parameter place retire and its place
/// leaves owned custody at every successful exit edge. This is checked
/// rather than asserted: `is_self` is admitted only on the declared
/// attachment type and only once, and a borrowed receiver carries `access`
/// other than `Owned`, so no parameter outside the consumed receiver can
/// route a claim through this rule. Claims rooted anywhere else remain
/// subject to the ordinary live-claim checks.
fn consume_terminal_self_receiver(
    machine: &terminal_psi::TerminalMachine,
    frontier: &mut StructuralOwnershipFrontier,
) {
    for parameter in &machine.structural_parameters {
        if parameter.is_self && parameter.access == StructuralAccess::Owned {
            frontier
                .claims
                .retain(|_, claim| claim.input != Some(parameter.place));
            frontier.owned_places.remove(&parameter.place);
        }
    }
}

/// Closes a block whose terminator is `ReturnUnit`.
fn close_return_unit(
    walk: &FrontierWalk<'_>,
    block: &terminal_psi::Block,
    mut frontier: StructuralOwnershipFrontier,
) -> Result<(), ModuleError> {
    let FrontierWalk {
        module,
        machine,
        parameter_order,
        ..
    } = *walk;
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &block.terminator
    else {
        unreachable!("dispatched close_return_unit")
    };
    if let Some(place) = frontier.partial_custody_paths.keys().next() {
        return Err(ModuleError::PartialStructuralCustodyAtUnitReturn {
            machine: machine.id,
            block: block.id,
            place: *place,
        });
    }
    if let Some(place) = frontier.restoration_debt.keys().next() {
        return Err(ModuleError::BorrowedStorageRestorationPending {
            machine: machine.id,
            block: block.id,
            place: *place,
        });
    }
    let expected_affine_discards =
        expected_trivial_affine_discards(machine, parameter_order, &frontier);
    if *trivial_affine_discards != expected_affine_discards {
        return Err(ModuleError::UnitReturnAffineDiscardsMismatch {
            machine: machine.id,
            block: block.id,
        });
    }
    release_reference_discards(
        module,
        machine,
        &mut frontier.references,
        trivial_affine_discards,
    )?;
    require_no_references(machine, &frontier.references)?;
    consume_terminal_self_receiver(machine, &mut frontier);
    if let Some((claim, _)) = frontier
        .claims
        .iter()
        .find(|(_, claim)| claim.multiplicity == Some(StructuralMultiplicity::Linear))
    {
        return Err(ModuleError::LiveLinearClaimAtUnitReturn {
            machine: machine.id,
            block: block.id,
            claim: *claim,
        });
    }
    Ok(())
}

/// Closes a block whose terminator is `ReturnUnitPartialAffine`.
fn close_return_unit_partial_affine(
    walk: &FrontierWalk<'_>,
    block: &terminal_psi::Block,
    mut frontier: StructuralOwnershipFrontier,
) -> Result<(), ModuleError> {
    let FrontierWalk {
        module,
        machine,
        parameter_order,
        ..
    } = *walk;
    let Terminator::ReturnUnitPartialAffine {
        trivial_affine_discards,
        residual_affine_discards,
        ..
    } = &block.terminator
    else {
        unreachable!("dispatched close_return_unit_partial_affine")
    };
    let Some(first_residual) = residual_affine_discards.first() else {
        return Err(ModuleError::InvalidPartialAffineCleanup {
            machine: machine.id,
            block: block.id,
        });
    };
    let root_place = first_residual.place;
    if residual_affine_discards
        .iter()
        .any(|residual| residual.place != root_place)
    {
        return Err(ModuleError::InvalidPartialAffineCleanup {
            machine: machine.id,
            block: block.id,
        });
    }
    let Some(moved) = frontier.partial_custody_paths.remove(&root_place) else {
        return Err(ModuleError::InvalidPartialAffineCleanup {
            machine: machine.id,
            block: block.id,
        });
    };
    let expected_residuals =
        partial_affine_root_type(machine, root_place).and_then(|structural_type| {
            partial_affine_residuals(
                module,
                structural_type,
                &moved,
                residual_affine_discards.len(),
            )
        });
    if moved.is_empty()
        || expected_residuals.as_ref().is_none_or(|expected| {
            residual_affine_discards.len() != expected.len()
                || residual_affine_discards.iter().zip(expected).any(
                    |(residual, (path, structural_type))| {
                        residual.path != *path || residual.structural_type != *structural_type
                    },
                )
        })
        || frontier.owned_places.remove(&root_place) != Some(StructuralMultiplicity::Affine)
    {
        return Err(ModuleError::InvalidPartialAffineCleanup {
            machine: machine.id,
            block: block.id,
        });
    }
    let expected_affine_discards =
        expected_trivial_affine_discards(machine, parameter_order, &frontier);
    if *trivial_affine_discards != expected_affine_discards {
        return Err(ModuleError::UnitReturnAffineDiscardsMismatch {
            machine: machine.id,
            block: block.id,
        });
    }
    if !frontier.partial_custody_paths.is_empty() {
        return Err(ModuleError::InvalidPartialAffineCleanup {
            machine: machine.id,
            block: block.id,
        });
    }
    if let Some(place) = frontier.restoration_debt.keys().next() {
        return Err(ModuleError::BorrowedStorageRestorationPending {
            machine: machine.id,
            block: block.id,
            place: *place,
        });
    }
    consume_terminal_self_receiver(machine, &mut frontier);
    if let Some((claim, _)) = frontier
        .claims
        .iter()
        .find(|(_, claim)| claim.multiplicity == Some(StructuralMultiplicity::Linear))
    {
        return Err(ModuleError::LiveLinearClaimAtUnitReturn {
            machine: machine.id,
            block: block.id,
            claim: *claim,
        });
    }
    Ok(())
}

/// Closes a block whose terminator is `ReturnUnitNominalAffine`.
fn close_return_unit_nominal_affine(
    walk: &FrontierWalk<'_>,
    block: &terminal_psi::Block,
    mut frontier: StructuralOwnershipFrontier,
) -> Result<(), ModuleError> {
    let FrontierWalk { machine, .. } = *walk;
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator else {
        unreachable!("dispatched close_return_unit_nominal_affine")
    };
    for cleanup in cleanups {
        if frontier
            .claims
            .values()
            .any(|claim| claim.input == Some(cleanup.place))
            || frontier.owned_places.remove(&cleanup.place) != Some(StructuralMultiplicity::Affine)
        {
            return Err(ModuleError::InvalidNominalAffineCleanup {
                machine: machine.id,
                block: block.id,
            });
        }
    }
    if let Some(place) = frontier.restoration_debt.keys().next() {
        return Err(ModuleError::BorrowedStorageRestorationPending {
            machine: machine.id,
            block: block.id,
            place: *place,
        });
    }
    consume_terminal_self_receiver(machine, &mut frontier);
    if !frontier.partial_custody_paths.is_empty()
        || !frontier.claims.is_empty()
        || !frontier.owned_places.is_empty()
    {
        return Err(ModuleError::InvalidNominalAffineCleanup {
            machine: machine.id,
            block: block.id,
        });
    }
    Ok(())
}

/// Closes a block whose terminator is `Return`.
fn close_return(
    walk: &FrontierWalk<'_>,
    block: &terminal_psi::Block,
    mut frontier: StructuralOwnershipFrontier,
) -> Result<(), ModuleError> {
    let FrontierWalk {
        module,
        machine,
        machines,
        parameter_order,
        ..
    } = *walk;
    let Terminator::Return {
        cleanup_actions, ..
    } = &block.terminator
    else {
        unreachable!("dispatched close_return")
    };
    consume_terminal_self_receiver(machine, &mut frontier);
    if let Some((claim, _)) = frontier
        .claims
        .iter()
        .find(|(_, claim)| claim.multiplicity == Some(StructuralMultiplicity::Linear))
    {
        return Err(ModuleError::LiveLinearClaimAtScalarReturn {
            machine: machine.id,
            block: block.id,
            claim: *claim,
        });
    }
    if let Some(place) = frontier.restoration_debt.keys().next() {
        return Err(ModuleError::BorrowedStorageRestorationPending {
            machine: machine.id,
            block: block.id,
            place: *place,
        });
    }
    validate_scalar_cleanup_actions(
        module,
        machine,
        parameter_order,
        machines,
        block.id,
        &frontier,
        cleanup_actions,
    )?;
    Ok(())
}

/// Closes a block whose terminator is `ReturnStructural`.
fn close_return_structural(
    walk: &FrontierWalk<'_>,
    block: &terminal_psi::Block,
    mut frontier: StructuralOwnershipFrontier,
) -> Result<(), ModuleError> {
    let FrontierWalk {
        module,
        machine,
        machines,
        parameter_order,
        ..
    } = *walk;
    let Terminator::ReturnStructural {
        source,
        returned_claims,
        trivial_affine_discards,
        ..
    } = &block.terminator
    else {
        unreachable!("dispatched close_return_structural")
    };
    let reference_return = super::super::references::transfer_return(
        module,
        machine,
        *source,
        &mut frontier.references,
    )?;
    if let Some(place) = frontier.restoration_debt.keys().next() {
        return Err(ModuleError::BorrowedStorageRestorationPending {
            machine: machine.id,
            block: block.id,
            place: *place,
        });
    }
    if frontier.partial_custody_paths.contains_key(source) {
        return Err(ModuleError::StructuralReturnSourcePartiallyMoved {
            machine: machine.id,
            block: block.id,
            place: *source,
        });
    }
    let result = machine
        .result
        .structural()
        .expect("control validation requires a structural result");
    let source_signature =
        super::super::structural_result_contracts::source_signature(machine, *source)
            .expect("control validation requires a structural source declaration");
    let plain_owned_block_return =
        super::super::block_views::plain_owned_return_source(module, machine, *source);
    let exact_unrestricted_parameter_return = source_signature.multiplicity
        == StructuralMultiplicity::Unrestricted
        && super::super::structural_result_contracts::has_empty_qualification_rosters(
            source_signature.qualifications,
            source_signature.projected_qualifications,
        )
        && matches!(machine.structural_parameters.as_slice(), [parameter]
            if parameter.place == *source
                && parameter.position == 0
                && !parameter.is_self
                && parameter.access == StructuralAccess::Owned);
    // Plain copy payloads owe no disposal. Their exact producer
    // dominance/order is checked by control-flow validation, not
    // by retaining a fictitious affine obligation across joins.
    if frontier.owned_places.remove(source).is_none()
        && !exact_unrestricted_parameter_return
        && !super::super::scalar_array::plain_return_source(module, machine, *source)
        && !(source_signature.multiplicity == StructuralMultiplicity::Unrestricted
            && (super::super::scalar_case::plain_return_source(module, machine, *source)
                || super::super::record::plain_return_source(module, machine, *source)))
        && !(plain_owned_block_return
            && source_signature.multiplicity == StructuralMultiplicity::Unrestricted)
    {
        return Err(ModuleError::StructuralReturnSourceNotLive {
            machine: machine.id,
            block: block.id,
            place: *source,
        });
    }
    if !super::super::structural_result_contracts::matches_function_result(source_signature, result)
    {
        return Err(ModuleError::StructuralReturnSignatureMismatch {
            machine: machine.id,
            block: block.id,
        });
    }
    let exact_payloadless_claim_free_return = returned_claims.is_empty()
        && source_signature.multiplicity == StructuralMultiplicity::Unrestricted
        && super::super::structural_result_contracts::has_empty_qualification_rosters(
            source_signature.qualifications,
            source_signature.projected_qualifications,
        )
        && machine
            .structural_places
            .iter()
            .find(|place| place.id == *source)
            .and_then(|place| match place.kind {
                StructuralPlaceKind::OperationResult { producer, .. } => machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|operation| operation.id == producer),
                _ => None,
            })
            .is_some_and(|operation| {
                (matches!(
                    &operation.kind,
                    OperationKind::EstablishScalarCase { fields, .. } if fields.is_empty()
                ) || super::super::structural_operations::exact_payloadless_structural_call(
                    module, operation, machines,
                )) && operation.result.structural().is_some_and(|result| {
                    result.place == *source
                        && result.multiplicity == StructuralMultiplicity::Unrestricted
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.claims.is_empty()
                })
            })
        || exact_unrestricted_parameter_return;
    let exact_affine_parameter_return = returned_claims.is_empty()
        && source_signature.multiplicity == StructuralMultiplicity::Affine
        && super::super::structural_result_contracts::has_empty_qualification_rosters(
            source_signature.qualifications,
            source_signature.projected_qualifications,
        )
        // The path-sensitive frontier above consumed this live root;
        // other parameters and preceding operations do not change
        // whether its exact result contract needs return claims.
        && machine.structural_parameters.iter().any(|parameter| {
            parameter.place == *source
                && !parameter.is_self
                && parameter.multiplicity == StructuralMultiplicity::Affine
                && parameter.access == StructuralAccess::Owned
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
        })
        && machine.entry_claims.is_empty()
        && machine.content_entry_claims.is_empty()
        && machine.published_service_ceiling.is_empty()
        && machine.contract.crash_routes.is_empty()
        && machine.contract.requires.is_empty()
        && machine.contract.ensures.is_empty()
        && machine.contract.outcome_specific_ensures.is_empty()
        && super::super::structural_result_contracts::has_plain_owned_shape(
            module,
            result.structural_type,
        );
    if (returned_claims.is_empty()
        && !reference_return
        && !exact_payloadless_claim_free_return
        && !exact_affine_parameter_return
        && !plain_owned_block_return
        && !super::super::structural_result_contracts::plain_owned_call_result(
            module, machine, *source,
        )
        && !super::super::scalar_array::plain_return_source(module, machine, *source)
        && !super::super::scalar_case::plain_return_source(module, machine, *source)
        && !super::super::record::plain_return_source(module, machine, *source))
        || returned_claims.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(ModuleError::NonCanonicalStructuralReturnClaims {
            machine: machine.id,
            block: block.id,
        });
    }
    let expected_claims = frontier
        .claims
        .iter()
        .filter_map(|(claim, live)| (live.input == Some(*source)).then_some(*claim))
        .collect::<Vec<_>>();
    if *returned_claims != expected_claims {
        return Err(ModuleError::StructuralReturnClaimSetMismatch {
            machine: machine.id,
            block: block.id,
        });
    }
    for claim in returned_claims {
        frontier.claims.remove(claim);
    }
    let expected_affine_discards =
        expected_trivial_affine_discards(machine, parameter_order, &frontier);
    if *trivial_affine_discards != expected_affine_discards {
        return Err(ModuleError::StructuralReturnAffineDiscardsMismatch {
            machine: machine.id,
            block: block.id,
        });
    }
    release_reference_discards(
        module,
        machine,
        &mut frontier.references,
        trivial_affine_discards,
    )?;
    require_no_references(machine, &frontier.references)?;
    // The consumed receiver's claims retire here only when it was not the
    // returned source: a `self` result's claims left through
    // `returned_claims` above and no longer key on its place.
    consume_terminal_self_receiver(machine, &mut frontier);
    if let Some(claim) = frontier.claims.keys().next() {
        return Err(ModuleError::LiveClaimAtStructuralReturn {
            machine: machine.id,
            block: block.id,
            claim: *claim,
        });
    }
    Ok(())
}

/// Closes a block whose terminator is `Crash`.
fn close_crash(
    block: &terminal_psi::Block,
    frontier: StructuralOwnershipFrontier,
) -> Result<(), ModuleError> {
    let Terminator::Crash {
        frontier_lower_bound,
        ..
    } = &block.terminator
    else {
        unreachable!("dispatched close_crash")
    };
    let expected = frontier.claims.keys().copied().collect::<Vec<_>>();
    if frontier_lower_bound != &expected {
        return Err(ModuleError::CrashFrontierMismatch { block: block.id });
    }
    Ok(())
}
