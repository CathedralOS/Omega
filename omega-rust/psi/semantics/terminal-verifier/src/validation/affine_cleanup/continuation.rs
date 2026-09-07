//! Exact projected transfers whose residual owner dies before a successor block.

use super::*;

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
    let mut root = None;
    let mut moved = BTreeSet::new();
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
        for argument in structural_arguments.iter().filter(|argument| {
            argument.access == StructuralAccess::Owned
                && !argument.path.is_empty()
                && partial_affine_root_type(machine, argument.place).is_some()
        }) {
            let root_type =
                partial_affine_root_type(machine, argument.place).ok_or_else(invalid)?;
            if root.is_some_and(|previous| previous != (argument.place, root_type))
                || structural_arguments.len() != 1
                || !claim_transfers.is_empty()
                || !moved.insert(argument.path.clone())
            {
                return Err(invalid());
            }
            root = Some((argument.place, root_type));
            let moved_type =
                resolve_structural_path(module, root_type, &argument.path).ok_or_else(invalid)?;
            let target = machines.get(callee).copied().ok_or_else(invalid)?;
            let [parameter] = target.structural_parameters.as_slice() else {
                return Err(invalid());
            };
            if target.result != TerminalMachineResult::Unit
                || !target.parameters.is_empty()
                || parameter.structural_type != moved_type
                || parameter.multiplicity != StructuralMultiplicity::Affine
                || parameter.position != 0
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
    let Some((place, root_type)) = root else {
        return if residual_affine_discards.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        };
    };
    // Mixed dying roots require one interleaved establishment-ordered schedule.
    // A separate root list cannot convey that order; unrelated live roots stay live.
    if !trivial_affine_discards.is_empty()
        || machine.result != TerminalMachineResult::Unit
        || (moved.iter().any(|path| {
            path.iter()
                .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
        }) && (!machine.contract.requires.is_empty()
            || !machine.contract.ensures.is_empty()
            || !machine.contract.crash_routes.is_empty()))
        || !machine.entry_claims.is_empty()
        || !machine.content_entry_claims.is_empty()
        || !machine.content_identity_reshuffles.is_empty()
        || !machine.content_partition_compositions.is_empty()
    {
        return Err(invalid());
    }
    let expected =
        partial_affine_residuals(module, root_type, &moved, residual_affine_discards.len())
            .ok_or_else(invalid)?;
    if residual_affine_discards.len() != expected.len()
        || residual_affine_discards.iter().zip(expected).any(
            |(residual, (path, structural_type))| {
                residual.place != place
                    || residual.path != path
                    || residual.structural_type != structural_type
            },
        )
    {
        return Err(invalid());
    }
    Ok(())
}
