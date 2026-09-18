//! Exact projected transfers whose residual owner dies before a successor block.

use super::{
    BTreeMap, BTreeSet, MachineId, ModuleError, OperationKind, PlaceId, StructuralAccess,
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeId, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, exact_fixed_array_element_sink,
    partial_affine_residuals, partial_affine_root_type, resolve_structural_path,
};
pub(super) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    block: &terminal_psi::Block,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let Terminator::Jump {
        trivial_affine_discards,
        residual_affine_discards,
        ..
    } = &block.terminator
    else {
        return Ok(());
    };
    let invalid = || ModuleError::InvalidPartialAffineCleanup {
        machine: machine.id,
        block: block.id,
    };
    // Every dying root keeps its own moved set. Roots appear in the order
    // their first projected operand does — the call lane's operand order —
    // which is also the order their residual complement groups appear in the
    // edge's discard list.
    let mut roots: Vec<PlaceId> = Vec::new();
    let mut moved: BTreeMap<PlaceId, (StructuralTypeId, BTreeSet<Vec<StructuralPathSegment>>)> =
        BTreeMap::new();
    let record_move = |roots: &mut Vec<PlaceId>,
                       moved: &mut BTreeMap<
        PlaceId,
        (StructuralTypeId, BTreeSet<Vec<StructuralPathSegment>>),
    >,
                       place: PlaceId,
                       path: &[StructuralPathSegment]|
     -> Result<StructuralTypeId, ModuleError> {
        let root_type = partial_affine_root_type(machine, place).ok_or_else(invalid)?;
        let (recorded_type, moved_paths) = moved
            .entry(place)
            .or_insert_with(|| (root_type, BTreeSet::new()));
        if *recorded_type != root_type || !moved_paths.insert(path.to_vec()) {
            return Err(invalid());
        }
        if moved_paths.len() == 1 {
            roots.push(place);
        }
        Ok(root_type)
    };
    let mut call_lane = false;
    for operation in &block.operations {
        let OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } = &operation.kind
        else {
            continue;
        };
        for (index, argument) in structural_arguments
            .iter()
            .enumerate()
            .filter(|(_, argument)| {
                argument.access == StructuralAccess::Owned
                    && !argument.path.is_empty()
                    && partial_affine_root_type(machine, argument.place).is_some()
            })
        {
            let root_type = record_move(&mut roots, &mut moved, argument.place, &argument.path)?;
            if !claim_transfers.is_empty() {
                return Err(invalid());
            }
            call_lane = true;
            let moved_type =
                resolve_structural_path(module, root_type, &argument.path).ok_or_else(invalid)?;
            let target = machines.get(callee).copied().ok_or_else(invalid)?;
            let Some(parameter) = target.structural_parameters.get(index) else {
                return Err(invalid());
            };
            if target.result != TerminalMachineResult::Unit
                || !target.parameters.is_empty()
                || parameter.structural_type != moved_type
                || parameter.multiplicity != StructuralMultiplicity::Affine
                || parameter.position != index as u32
                || parameter.is_self
                || parameter.access != StructuralAccess::Owned
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || (argument
                    .path
                    .iter()
                    .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
                    && !exact_fixed_array_element_sink(target, moved_type))
            {
                return Err(invalid());
            }
        }
    }
    // A Jump edge may also move a projected affine child: the selected child
    // becomes an ordinary owned parameter of the continuation block while the
    // residual complement dies on this edge. The successor parameter checks
    // mirror the whole-source contract the frontier enforces positionally.
    if let Terminator::Jump {
        target,
        structural_arguments,
        ..
    } = &block.terminator
    {
        // An absent target block and a successor arity mismatch belong to
        // `control_flow` (`UnknownTargetBlock`) and to the frontier's
        // positional consume (`StructuralJumpArityMismatch`), which both run
        // after this shape pre-pass. Rejecting them here only replaces their
        // exact diagnostic with a cleanup-shape one; the module still fails
        // there, so this pass leaves the projected-child contract unchecked
        // on an edge whose parameter roster is not yet known to line up.
        let Some(target_block) = machine
            .blocks
            .iter()
            .find(|candidate| candidate.id == *target)
            .filter(|target_block| {
                structural_arguments.len() == target_block.structural_parameters.len()
            })
        else {
            return Ok(());
        };
        for (argument, parameter) in structural_arguments
            .iter()
            .zip(&target_block.structural_parameters)
        {
            if argument.access != StructuralAccess::Owned || argument.path.is_empty() {
                continue;
            }
            let root_type = record_move(&mut roots, &mut moved, argument.place, &argument.path)?;
            let moved_type =
                resolve_structural_path(module, root_type, &argument.path).ok_or_else(invalid)?;
            if parameter.structural_type != moved_type
                || parameter.multiplicity != StructuralMultiplicity::Affine
                || parameter.is_self
                || parameter.access != StructuralAccess::Owned
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(invalid());
            }
        }
    }
    if roots.is_empty() {
        return if residual_affine_discards.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    // Each dying root's residual complement keeps its canonical order; the
    // groups appear in the order their roots were first projected. The
    // Jump-edge lane still composes with the edge's own unselected-owner
    // cleanup: a trivial discard there names a different live root and keeps
    // its ordinary order, while the residual list closes exactly the
    // projected roots' complements.
    if (call_lane
        && (!trivial_affine_discards.is_empty() || machine.result != TerminalMachineResult::Unit))
        || (moved
            .values()
            .flat_map(|(_, paths)| paths.iter())
            .any(|path| {
                path.iter()
                    .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
            })
            && (!machine.contract.requires.is_empty()
                || !machine.contract.ensures.is_empty()
                || !machine.contract.crash_routes.is_empty()))
        || !machine.entry_claims.is_empty()
        || !machine.content_entry_claims.is_empty()
        || !machine.content_identity_reshuffles.is_empty()
        || !machine.content_partition_compositions.is_empty()
    {
        return Err(invalid());
    }
    let mut expected = Vec::new();
    for place in &roots {
        let (root_type, moved_paths) = moved.get(place).ok_or_else(invalid)?;
        for (path, structural_type) in partial_affine_residuals(
            module,
            *root_type,
            moved_paths,
            residual_affine_discards.len(),
        )
        .ok_or_else(invalid)?
        {
            expected.push((*place, path, structural_type));
        }
    }
    if residual_affine_discards.len() != expected.len()
        || residual_affine_discards.iter().zip(&expected).any(
            |(residual, (place, path, structural_type))| {
                residual.place != *place
                    || residual.path != *path
                    || residual.structural_type != *structural_type
            },
        )
    {
        return Err(invalid());
    }
    Ok(())
}
