//! Resolve call effects from retained placement before the common effect checks.
//!
//! Canonical catalogs own instruction mechanisms, not a Cartesian product of
//! source signatures. The selected call roster supplies the evaluated plan; the
//! target template supplies clobbers and control. Both analysis and receiving
//! validation reconstruct this join and compare the complete selected operands.

use std::borrow::Cow;

use register_model::{
    RegisterInstructionConstraint, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
};
use selected_instructions::{
    MachineEffectDeclaration, SelectedFunction, SelectedInstruction, SelectedInstructionKind,
};

use super::MachineEffectError;

pub(super) fn constraint<'catalog>(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &'catalog ValidatedRegisterConstraintCatalog,
) -> Result<Cow<'catalog, RegisterInstructionConstraint>, MachineEffectError> {
    let invalid = || MachineEffectError::ConstraintEffectMismatch {
        instruction: instruction.id,
    };
    let base = catalog
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == instruction.constraint)
        .ok_or_else(invalid)?;
    if !matches!(
        instruction.kind,
        SelectedInstructionKind::NormalizedForeignCall { .. }
    ) {
        return Ok(Cow::Borrowed(base));
    }
    let record =
        selected_instructions::calls::constraints::foreign_call_record(function, instruction)
            .ok_or_else(invalid)?;
    let row = selected_instructions::calls::constraints::instantiate_foreign_call_constraint(
        &record.call,
        base,
        physical,
        |register| register_environment::fixed_register_view(physical, register),
    )
    .ok_or_else(invalid)?;
    Ok(Cow::Owned(row))
}

pub(super) fn declaration<'catalog>(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    constraint: &RegisterInstructionConstraint,
    base: &'catalog MachineEffectDeclaration,
) -> Result<Cow<'catalog, MachineEffectDeclaration>, MachineEffectError> {
    if !matches!(
        instruction.kind,
        SelectedInstructionKind::NormalizedForeignCall { .. }
    ) {
        return Ok(Cow::Borrowed(base));
    }
    let invalid = || MachineEffectError::ConstraintEffectMismatch {
        instruction: instruction.id,
    };
    let record =
        selected_instructions::calls::constraints::foreign_call_record(function, instruction)
            .ok_or_else(invalid)?;
    let declaration = match physical.model().architecture {
        target::Architecture::Aarch64 => {
            isa_aarch64::aarch64_normalized_foreign_call_declaration(record, constraint)
        }
        target::Architecture::X86_64 => {
            isa_x86_64::x86_64_normalized_foreign_call_declaration(record, constraint)
        }
    }
    .ok_or_else(invalid)?;
    if declaration.semantic != base.semantic || declaration.constraint != base.constraint {
        return Err(invalid());
    }
    Ok(Cow::Owned(declaration))
}
