use register_model::{RegisterOperandAccess, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedInstruction,
};
use semantic_vocabulary::IntegerValue;

use crate::{Aarch64MovnMaterializationError, QualifiedPhysicalWrite};
use physical_instructions::{PhysicalOperandFootprint, PostAllocationMachineInstruction};

pub(super) fn integer_bits(
    value: IntegerValue,
    instruction: selected_instructions::SelectedInstructionId,
) -> Result<u64, Aarch64MovnMaterializationError> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value)
            .map(|value| value as u64)
            .map_err(|_| Aarch64MovnMaterializationError::IntegerOutsideI64Bits(instruction)),
        IntegerValue::Unsigned(value) => u64::try_from(value)
            .map_err(|_| Aarch64MovnMaterializationError::IntegerOutsideI64Bits(instruction)),
    }
}

pub(super) fn validate_materialization(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64MovnMaterializationError> {
    let encoded = &machine.alternative.encoded;
    if machine.instruction != selected.id
        || selected.operands.len() != 1
        || !selected.implicit_uses.is_empty()
        || !selected.implicit_defs.is_empty()
        || !selected.clobbers.is_empty()
        || machine.alternative.key.family != MachineAlternativeFamily::MaterializeI64
        || machine.alternative.key.variant != 0
        || !encoded.external_operand_reads.is_empty()
        || encoded.external_operand_writes != [0]
        || !encoded.implicit_unit_uses.is_empty()
        || !encoded.implicit_unit_defs.is_empty()
        || !encoded.implicit_unit_clobbers.is_empty()
        || encoded.memory != MachineEncodedMemoryEffect::NoneV1
        || encoded.stack != MachineEncodedStackEffect::UnchangedV1
        || encoded.trap != MachineEncodedTrapBehavior::NeverV1
        || encoded.control != MachineEncodedControlEffect::FallThroughV1
        || machine.operands.len() != 1
        || !machine.implicit_unit_uses.is_empty()
        || !machine.implicit_unit_defs.is_empty()
        || !machine.implicit_unit_clobbers.is_empty()
        || !machine.unit_uses.is_empty()
        || !machine.unit_clobbers.is_empty()
    {
        return Err(Aarch64MovnMaterializationError::InvalidMaterializationFootprint(selected.id));
    }
    let selected_operand = &selected.operands[0];
    let operand = &machine.operands[0];
    if selected_operand.operand != 0
        || selected_operand.access != RegisterOperandAccess::Def
        || operand.operand != 0
        || operand.virtual_register != selected_operand.virtual_register
        || operand.class != selected_operand.class
        || operand.access != RegisterOperandAccess::Def
        || !operand.read_units.is_empty()
        || machine.unit_defs != operand.write_units
        || operand.write_semantics.is_none()
    {
        return Err(Aarch64MovnMaterializationError::InvalidMaterializationFootprint(selected.id));
    }
    validate_x_view(operand, physical, selected.id)
}

fn validate_x_view(
    operand: &PhysicalOperandFootprint,
    physical: &ValidatedPhysicalRegisterModel,
    instruction: selected_instructions::SelectedInstructionId,
) -> Result<(), Aarch64MovnMaterializationError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == operand.view)
        .ok_or(Aarch64MovnMaterializationError::InvalidPhysicalDestination(
            instruction,
        ))?;
    let valid_index = view
        .name
        .strip_prefix('x')
        .and_then(|name| name.parse::<u8>().ok())
        .is_some_and(|index| index <= 30);
    if !valid_index
        || view.bits != 64
        || !view.allocatable
        || view.class != operand.class
        || view.units != operand.storage_units
        || view.write_units != operand.write_units
        || Some(view.write_semantics) != operand.write_semantics
    {
        return Err(Aarch64MovnMaterializationError::InvalidPhysicalDestination(
            instruction,
        ));
    }
    Ok(())
}

pub(super) fn qualified_write(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
) -> Result<QualifiedPhysicalWrite, Aarch64MovnMaterializationError> {
    let operand = machine
        .operands
        .first()
        .ok_or(Aarch64MovnMaterializationError::InvalidMaterializationFootprint(selected.id))?;
    Ok(QualifiedPhysicalWrite {
        instruction: selected.id,
        operand: operand.operand,
        virtual_register: operand.virtual_register,
        class: operand.class,
        view: operand.view,
        storage_units: operand.storage_units.clone(),
        write_units: operand.write_units.clone(),
        write_semantics: operand
            .write_semantics
            .ok_or(Aarch64MovnMaterializationError::InvalidMaterializationFootprint(selected.id))?,
    })
}
