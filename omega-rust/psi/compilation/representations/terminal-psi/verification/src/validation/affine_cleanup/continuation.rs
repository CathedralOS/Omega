//! Static contracts for projected successor operands and residual declarations.
//! The ownership frontier, not the location of a transfer operation, determines
//! which children remain live when a later continuation disposes their root.

use super::{
    ModuleError, StructuralAccess, StructuralMultiplicity, TerminalMachine, TerminalModule,
    Terminator, partial_affine_root_type, resolve_structural_path,
};

pub(super) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    block: &terminal_psi::Block,
) -> Result<(), ModuleError> {
    let Terminator::Jump {
        target,
        structural_arguments,
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
    for residual in residual_affine_discards {
        let root = partial_affine_root_type(machine, residual.place).ok_or_else(invalid)?;
        if residual.path.is_empty()
            || resolve_structural_path(module, root, &residual.path)
                != Some(residual.structural_type)
        {
            return Err(invalid());
        }
    }
    // Missing targets and arity mismatches retain their control-flow diagnostics.
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
        let root = partial_affine_root_type(machine, argument.place).ok_or_else(invalid)?;
        let selected = resolve_structural_path(module, root, &argument.path).ok_or_else(invalid)?;
        if parameter.structural_type != selected
            || parameter.multiplicity != StructuralMultiplicity::Affine
            || parameter.is_self
            || parameter.access != StructuralAccess::Owned
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return Err(invalid());
        }
    }
    Ok(())
}
