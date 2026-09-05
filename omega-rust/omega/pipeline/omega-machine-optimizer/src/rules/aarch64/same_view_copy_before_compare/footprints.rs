use std::collections::BTreeSet;

use omega_register_model::{RegisterOperandAccess, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedInstruction,
    SelectedInstructionKind,
};
use omega_selected_instructions_to_register_homes::{BlockLiveness, InstructionLiveness};

use crate::{
    Aarch64SameViewCopyElisionError, PostAllocationMachineInstruction, QualifiedPhysicalOperand,
};

use super::CompareConsumerContract;

pub(super) struct PairEvidence {
    pub source: QualifiedPhysicalOperand,
    pub destination: QualifiedPhysicalOperand,
    pub consumed: QualifiedPhysicalOperand,
    pub same_storage: bool,
    pub destination_consumed: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_pair(
    copy: &SelectedInstruction,
    compare: &SelectedInstruction,
    machine_copy: &PostAllocationMachineInstruction,
    machine_compare: &PostAllocationMachineInstruction,
    live_copy: &InstructionLiveness,
    live_compare: &InstructionLiveness,
    _live_block: &BlockLiveness,
    physical: &ValidatedPhysicalRegisterModel,
    contract: CompareConsumerContract,
) -> Result<PairEvidence, Aarch64SameViewCopyElisionError> {
    validate_copy(copy, machine_copy, physical)?;
    validate_compare(compare, machine_compare, physical, contract)?;
    if live_copy.instruction != copy.id
        || live_compare.instruction != compare.id
        || live_copy.unit_live_out != live_compare.unit_live_in
    {
        return Err(Aarch64SameViewCopyElisionError::LivenessRosterMismatch(
            compare.id,
        ));
    }
    let source = qualified(machine_copy, 0, copy.id)?;
    let destination = qualified(machine_copy, 1, copy.id)?;
    let consumed = qualified(machine_compare, contract.consumed_operand, compare.id)?;
    if !consumed.storage_units.iter().all(|unit| {
        live_copy.unit_live_out.contains(unit)
            && live_compare.unit_live_in.contains(unit)
            && live_compare.unit_uses.contains(unit)
    }) {
        return Err(Aarch64SameViewCopyElisionError::LivenessRosterMismatch(
            compare.id,
        ));
    }
    Ok(PairEvidence {
        same_storage: source.view == destination.view
            && source.storage_units == destination.storage_units,
        destination_consumed: destination.virtual_register == consumed.virtual_register,
        source,
        destination,
        consumed,
    })
}

fn validate_copy(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64SameViewCopyElisionError> {
    let encoded = &machine.alternative.encoded;
    if machine.instruction != selected.id
        || !matches!(selected.kind, SelectedInstructionKind::CopyI64)
        || selected.operands.len() != 2
        || machine.alternative.key.family != MachineAlternativeFamily::CopyI64
        || machine.alternative.key.variant != 0
        || encoded.external_operand_reads != [0]
        || encoded.external_operand_writes != [1]
        || !encoded.implicit_unit_uses.is_empty()
        || !encoded.implicit_unit_defs.is_empty()
        || !encoded.implicit_unit_clobbers.is_empty()
        || encoded.memory != MachineEncodedMemoryEffect::NoneV1
        || encoded.stack != MachineEncodedStackEffect::UnchangedV1
        || encoded.trap != MachineEncodedTrapBehavior::NeverV1
        || encoded.control != MachineEncodedControlEffect::FallThroughV1
        || !machine.implicit_unit_uses.is_empty()
        || !machine.implicit_unit_defs.is_empty()
        || !machine.implicit_unit_clobbers.is_empty()
        || machine.operands.len() != 2
    {
        return Err(Aarch64SameViewCopyElisionError::InvalidCopyFootprint(
            selected.id,
        ));
    }
    validate_operand(selected, machine, 0, RegisterOperandAccess::Use, physical)?;
    validate_operand(selected, machine, 1, RegisterOperandAccess::Def, physical)?;
    validate_whole_units(machine, selected.id)
}

fn validate_compare(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    contract: CompareConsumerContract,
) -> Result<(), Aarch64SameViewCopyElisionError> {
    let nzcv = physical.model().view_named("nzcv").ok_or(
        Aarch64SameViewCopyElisionError::MissingArchitecturalView("nzcv"),
    )?;
    let encoded = &machine.alternative.encoded;
    if machine.instruction != selected.id
        || selected.kind != contract.kind
        || selected.operands.len() != contract.operand_count
        || machine.alternative.key.family != contract.family
        || machine.alternative.key.variant != 0
        || encoded.external_operand_reads != contract.external_reads
        || !encoded.external_operand_writes.is_empty()
        || !encoded.implicit_unit_uses.is_empty()
        || encoded.implicit_unit_defs != nzcv.units
        || !encoded.implicit_unit_clobbers.is_empty()
        || encoded.memory != MachineEncodedMemoryEffect::NoneV1
        || encoded.stack != MachineEncodedStackEffect::UnchangedV1
        || encoded.trap != MachineEncodedTrapBehavior::NeverV1
        || encoded.control != MachineEncodedControlEffect::FallThroughV1
        || !machine.implicit_unit_uses.is_empty()
        || machine.implicit_unit_defs != nzcv.units
        || !machine.implicit_unit_clobbers.is_empty()
        || machine.operands.len() != contract.operand_count
    {
        return Err(Aarch64SameViewCopyElisionError::InvalidCompareFootprint(
            selected.id,
        ));
    }
    for ordinal in 0..contract.operand_count {
        validate_operand(
            selected,
            machine,
            ordinal,
            RegisterOperandAccess::Use,
            physical,
        )?;
    }
    validate_whole_units(machine, selected.id)
}

fn validate_operand(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    ordinal: usize,
    access: RegisterOperandAccess,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64SameViewCopyElisionError> {
    let selected_operand = selected.operands.get(ordinal).ok_or(
        Aarch64SameViewCopyElisionError::InvalidPhysicalOperand(selected.id),
    )?;
    let operand = machine.operands.get(ordinal).ok_or(
        Aarch64SameViewCopyElisionError::InvalidPhysicalOperand(selected.id),
    )?;
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == operand.view)
        .ok_or(Aarch64SameViewCopyElisionError::InvalidPhysicalOperand(
            selected.id,
        ))?;
    let valid_x = view
        .name
        .strip_prefix('x')
        .and_then(|name| name.parse::<u8>().ok())
        .is_some_and(|index| index <= 30);
    let read_matches = match access {
        RegisterOperandAccess::Use => operand.read_units == operand.storage_units,
        RegisterOperandAccess::Def => operand.read_units.is_empty(),
        RegisterOperandAccess::UseDef => false,
    };
    let write_matches = match access {
        RegisterOperandAccess::Use => {
            operand.write_units.is_empty() && operand.write_semantics.is_none()
        }
        RegisterOperandAccess::Def => {
            operand.write_units == view.write_units
                && operand.write_semantics == Some(view.write_semantics)
        }
        RegisterOperandAccess::UseDef => false,
    };
    if selected_operand.operand != u16::try_from(ordinal).expect("bounded operand")
        || selected_operand.access != access
        || selected_operand.fixed_view.is_some()
        || selected_operand.tied_to.is_some()
        || selected_operand.early_clobber
        || operand.operand != selected_operand.operand
        || operand.virtual_register != selected_operand.virtual_register
        || operand.class != selected_operand.class
        || operand.access != access
        || !valid_x
        || view.bits != 64
        || !view.allocatable
        || view.class != operand.class
        || view.units != operand.storage_units
        || !read_matches
        || !write_matches
    {
        return Err(Aarch64SameViewCopyElisionError::InvalidPhysicalOperand(
            selected.id,
        ));
    }
    Ok(())
}

fn validate_whole_units(
    machine: &PostAllocationMachineInstruction,
    instruction: omega_selected_instructions::SelectedInstructionId,
) -> Result<(), Aarch64SameViewCopyElisionError> {
    let uses = machine
        .operands
        .iter()
        .flat_map(|operand| &operand.read_units)
        .chain(&machine.implicit_unit_uses)
        .copied()
        .collect::<BTreeSet<_>>();
    let defs = machine
        .operands
        .iter()
        .flat_map(|operand| &operand.write_units)
        .chain(&machine.implicit_unit_defs)
        .copied()
        .collect::<BTreeSet<_>>();
    if machine.unit_uses.iter().copied().collect::<BTreeSet<_>>() != uses
        || machine.unit_defs.iter().copied().collect::<BTreeSet<_>>() != defs
        || machine.unit_clobbers != machine.implicit_unit_clobbers
    {
        return Err(Aarch64SameViewCopyElisionError::InvalidPhysicalOperand(
            instruction,
        ));
    }
    Ok(())
}

fn qualified(
    machine: &PostAllocationMachineInstruction,
    ordinal: usize,
    instruction: omega_selected_instructions::SelectedInstructionId,
) -> Result<QualifiedPhysicalOperand, Aarch64SameViewCopyElisionError> {
    let operand = machine.operands.get(ordinal).ok_or(
        Aarch64SameViewCopyElisionError::InvalidPhysicalOperand(instruction),
    )?;
    Ok(QualifiedPhysicalOperand {
        instruction,
        operand: operand.operand,
        virtual_register: operand.virtual_register,
        class: operand.class,
        view: operand.view,
        storage_units: operand.storage_units.clone(),
    })
}
