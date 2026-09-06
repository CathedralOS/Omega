use std::collections::BTreeSet;

use register_model::{RegisterOperandAccess, ValidatedPhysicalRegisterModel};
use selected_instructions::{BlockLiveness, InstructionLiveness};
use selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedInstruction,
    SelectedInstructionKind,
};

use crate::{Aarch64SameViewCopyElisionError, QualifiedPhysicalOperand};
use physical_instructions::PostAllocationMachineInstruction;

pub(super) struct PairEvidence {
    pub source: QualifiedPhysicalOperand,
    pub destination: QualifiedPhysicalOperand,
    pub returned: QualifiedPhysicalOperand,
    pub same_storage: bool,
    pub destination_returned: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_pair(
    copy: &SelectedInstruction,
    returned: &SelectedInstruction,
    machine_copy: &PostAllocationMachineInstruction,
    machine_return: &PostAllocationMachineInstruction,
    live_copy: &InstructionLiveness,
    live_return: &InstructionLiveness,
    live_block: &BlockLiveness,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<PairEvidence, Aarch64SameViewCopyElisionError> {
    validate_copy(copy, machine_copy, physical)?;
    validate_return(returned, machine_return, physical)?;
    if live_copy.instruction != copy.id
        || live_return.instruction != returned.id
        || live_copy.unit_live_out != live_return.unit_live_in
    {
        return Err(Aarch64SameViewCopyElisionError::LivenessRosterMismatch(
            returned.id,
        ));
    }
    let source = qualified(machine_copy, 0, copy.id)?;
    let destination = qualified(machine_copy, 1, copy.id)?;
    let returned_operand = qualified(machine_return, 0, returned.id)?;
    if !returned_operand.storage_units.iter().all(|unit| {
        live_copy.unit_live_out.contains(unit)
            && live_return.unit_live_in.contains(unit)
            && live_return.unit_uses.contains(unit)
    }) || !live_block.successors.is_empty()
    {
        return Err(Aarch64SameViewCopyElisionError::LivenessRosterMismatch(
            returned.id,
        ));
    }
    Ok(PairEvidence {
        same_storage: source.view == destination.view
            && source.storage_units == destination.storage_units,
        destination_returned: destination.virtual_register == returned_operand.virtual_register,
        source,
        destination,
        returned: returned_operand,
    })
}

fn validate_copy(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64SameViewCopyElisionError> {
    if machine.instruction != selected.id
        || !matches!(selected.kind, SelectedInstructionKind::CopyI64)
        || selected.operands.len() != 2
        || machine.alternative.key.family != MachineAlternativeFamily::CopyI64
        || machine.alternative.key.variant != 0
        || machine.alternative.encoded.external_operand_reads != [0]
        || machine.alternative.encoded.external_operand_writes != [1]
        || !machine.alternative.encoded.implicit_unit_uses.is_empty()
        || !machine.alternative.encoded.implicit_unit_defs.is_empty()
        || !machine
            .alternative
            .encoded
            .implicit_unit_clobbers
            .is_empty()
        || machine.alternative.encoded.memory != MachineEncodedMemoryEffect::NoneV1
        || machine.alternative.encoded.stack != MachineEncodedStackEffect::UnchangedV1
        || machine.alternative.encoded.trap != MachineEncodedTrapBehavior::NeverV1
        || machine.alternative.encoded.control != MachineEncodedControlEffect::FallThroughV1
        || !machine.implicit_unit_uses.is_empty()
        || !machine.implicit_unit_defs.is_empty()
        || !machine.implicit_unit_clobbers.is_empty()
        || machine.operands.len() != 2
    {
        return Err(Aarch64SameViewCopyElisionError::InvalidCopyFootprint(
            selected.id,
        ));
    }
    validate_operand(
        selected,
        machine,
        0,
        RegisterOperandAccess::Use,
        None,
        physical,
    )?;
    validate_operand(
        selected,
        machine,
        1,
        RegisterOperandAccess::Def,
        None,
        physical,
    )?;
    validate_whole_units(machine, selected.id)
}

fn validate_return(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64SameViewCopyElisionError> {
    let x0 = named_view(physical, "x0", selected.id)?;
    let x30 = named_view(physical, "x30", selected.id)?;
    let pc = named_view(physical, "pc", selected.id)?;
    let encoded = &machine.alternative.encoded;
    if machine.instruction != selected.id
        || !matches!(selected.kind, SelectedInstructionKind::ReturnI64)
        || selected.operands.len() != 1
        || machine.alternative.key.family != MachineAlternativeFamily::ReturnI64
        || machine.alternative.key.variant != 0
        || !encoded.external_operand_reads.is_empty()
        || !encoded.external_operand_writes.is_empty()
        || encoded.implicit_unit_uses != x30.units
        || encoded.implicit_unit_defs != pc.units
        || !encoded.implicit_unit_clobbers.is_empty()
        || encoded.memory != MachineEncodedMemoryEffect::NoneV1
        || encoded.stack != MachineEncodedStackEffect::UnchangedV1
        || encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        || encoded.control
            != (MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target: x30.id })
        || machine.implicit_unit_uses != x30.units
        || machine.implicit_unit_defs != pc.units
        || !machine.implicit_unit_clobbers.is_empty()
        || machine.operands.len() != 1
    {
        return Err(Aarch64SameViewCopyElisionError::InvalidReturnFootprint(
            selected.id,
        ));
    }
    validate_operand(
        selected,
        machine,
        0,
        RegisterOperandAccess::Use,
        Some(x0.id),
        physical,
    )?;
    validate_whole_units(machine, selected.id)
}

fn validate_operand(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    ordinal: usize,
    access: RegisterOperandAccess,
    fixed: Option<register_model::RegisterViewId>,
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
    if selected_operand.operand != u16::try_from(ordinal).unwrap()
        || selected_operand.access != access
        || selected_operand.fixed_view != fixed
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
        || fixed.is_some_and(|fixed| fixed != operand.view)
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
    instruction: selected_instructions::SelectedInstructionId,
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
    instruction: selected_instructions::SelectedInstructionId,
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

fn named_view<'a>(
    physical: &'a ValidatedPhysicalRegisterModel,
    name: &'static str,
    instruction: selected_instructions::SelectedInstructionId,
) -> Result<&'a register_model::RegisterView, Aarch64SameViewCopyElisionError> {
    physical
        .model()
        .view_named(name)
        .ok_or(Aarch64SameViewCopyElisionError::MissingArchitecturalView(
            name,
        ))
        .map_err(|error| match error {
            Aarch64SameViewCopyElisionError::MissingArchitecturalView(_) => error,
            _ => Aarch64SameViewCopyElisionError::InvalidPhysicalOperand(instruction),
        })
}
