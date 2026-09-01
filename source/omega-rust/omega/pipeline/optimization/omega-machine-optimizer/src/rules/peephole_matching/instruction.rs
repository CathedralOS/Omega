use std::collections::BTreeSet;

use omega_regalloc::InstructionLiveness;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{
    MachineSemanticKind, SelectedInstruction, SelectedInstructionKind,
};

use crate::{PhysicalOperandFootprint, PostAllocationMachineInstruction};

use super::{
    InstructionPattern, MatchedPhysicalRead, TerminalPairMatchError, ViewPattern,
    model::ResolvedNamedUnitSet, registers,
};

pub(super) fn match_instruction(
    pattern: &InstructionPattern,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    live: &InstructionLiveness,
    physical: &ValidatedPhysicalRegisterModel,
    named: &[ResolvedNamedUnitSet],
    first: bool,
) -> Result<Vec<MatchedPhysicalRead>, TerminalPairMatchError> {
    if machine.instruction != selected.id
        || live.instruction != selected.id
        || semantic(&selected.kind) != pattern.semantic
        || selected.operands.len() != pattern.selected_operand_count
    {
        return Err(roster_error(first, selected.id));
    }
    let encoded = &machine.alternative.encoded;
    let uses = registers::units_for(pattern.implicit_uses, named);
    let defs = registers::units_for(pattern.implicit_defs, named);
    let clobbers = registers::units_for(pattern.implicit_clobbers, named);
    if machine.alternative.key.family != pattern.family
        || machine.alternative.key.variant != pattern.variant
        || encoded.external_operand_reads != pattern.external_reads
        || encoded.external_operand_writes != pattern.external_writes
        || encoded.implicit_unit_uses != uses
        || encoded.implicit_unit_defs != defs
        || encoded.implicit_unit_clobbers != clobbers
        || encoded.memory != pattern.memory
        || encoded.stack != pattern.stack
        || encoded.trap != pattern.trap
        || encoded.control != pattern.control
        || machine.implicit_unit_uses != uses
        || machine.implicit_unit_defs != defs
        || machine.implicit_unit_clobbers != clobbers
        || machine.operands.len() != pattern.operands.len()
    {
        return Err(footprint_error(first, selected.id));
    }
    let reads = pattern
        .operands
        .iter()
        .map(|operand_pattern| match_operand(operand_pattern, selected, machine, physical, first))
        .collect::<Result<Vec<_>, _>>()?;
    if !whole_instruction_units_match(machine) {
        return Err(footprint_error(first, selected.id));
    }
    Ok(reads)
}

fn match_operand(
    pattern: &super::model::OperandPattern,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    first: bool,
) -> Result<MatchedPhysicalRead, TerminalPairMatchError> {
    let selected_operand = selected
        .operands
        .iter()
        .find(|operand| operand.operand == pattern.operand)
        .ok_or_else(|| footprint_error(first, selected.id))?;
    let operand = machine
        .operands
        .iter()
        .find(|operand| operand.operand == pattern.operand)
        .ok_or_else(|| footprint_error(first, selected.id))?;
    if selected_operand.access != pattern.access
        || operand.virtual_register != selected_operand.virtual_register
        || operand.class != selected_operand.class
        || operand.access != pattern.access
        || (pattern.read_equals_storage && operand.read_units != operand.storage_units)
        || (pattern.writes_empty && !operand.write_units.is_empty())
        || (pattern.no_write_semantics && operand.write_semantics.is_some())
    {
        return Err(footprint_error(first, selected.id));
    }
    match_view(pattern.view, operand, physical).map_err(|()| physical_error(first, selected.id))?;
    Ok(MatchedPhysicalRead {
        source_instruction: selected.id,
        operand: operand.operand,
        virtual_register: operand.virtual_register,
        class: operand.class,
        view: operand.view,
        units: operand.read_units.clone(),
    })
}

fn match_view(
    pattern: ViewPattern,
    operand: &PhysicalOperandFootprint,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), ()> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == operand.view)
        .ok_or(())?;
    match pattern {
        ViewPattern::IndexedAllocatable {
            prefix,
            maximum_index,
            bits,
        } => {
            let valid_index = view
                .name
                .strip_prefix(prefix)
                .and_then(|name| name.parse::<u8>().ok())
                .is_some_and(|index| index <= maximum_index);
            if !valid_index
                || view.bits != bits
                || !view.allocatable
                || view.class != operand.class
                || view.units != operand.storage_units
            {
                return Err(());
            }
        }
    }
    Ok(())
}

fn whole_instruction_units_match(machine: &PostAllocationMachineInstruction) -> bool {
    let operand_uses = machine
        .operands
        .iter()
        .flat_map(|operand| &operand.read_units);
    let operand_defs = machine
        .operands
        .iter()
        .flat_map(|operand| &operand.write_units);
    let uses = operand_uses
        .chain(&machine.implicit_unit_uses)
        .copied()
        .collect::<BTreeSet<_>>();
    let defs = operand_defs
        .chain(&machine.implicit_unit_defs)
        .copied()
        .collect::<BTreeSet<_>>();
    machine.unit_uses.iter().copied().collect::<BTreeSet<_>>() == uses
        && machine.unit_defs.iter().copied().collect::<BTreeSet<_>>() == defs
        && machine.unit_clobbers == machine.implicit_unit_clobbers
}

fn semantic(kind: &SelectedInstructionKind) -> MachineSemanticKind {
    match kind {
        SelectedInstructionKind::CompareI64Zero => MachineSemanticKind::CompareI64Zero,
        SelectedInstructionKind::MaterializeI64 { .. } => MachineSemanticKind::MaterializeI64,
        SelectedInstructionKind::CopyI64 => MachineSemanticKind::CopyI64,
        SelectedInstructionKind::ExactAddI64 { .. } => MachineSemanticKind::ExactAddI64,
        SelectedInstructionKind::ExactAddI64Immediate { .. } => {
            MachineSemanticKind::ExactAddI64Immediate
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => MachineSemanticKind::ExactSubtractI64,
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
            MachineSemanticKind::ExactSubtractI64Immediate
        }
        SelectedInstructionKind::ConditionalBranchNonZero => {
            MachineSemanticKind::ConditionalBranchNonZero
        }
        SelectedInstructionKind::ReturnI64 => MachineSemanticKind::ReturnI64,
        SelectedInstructionKind::ReturnUnit => MachineSemanticKind::ReturnUnit,
    }
}

fn roster_error(
    first: bool,
    id: omega_selected_instructions::SelectedInstructionId,
) -> TerminalPairMatchError {
    if first {
        TerminalPairMatchError::FirstRoster(id)
    } else {
        TerminalPairMatchError::SecondRoster(id)
    }
}

fn footprint_error(
    first: bool,
    id: omega_selected_instructions::SelectedInstructionId,
) -> TerminalPairMatchError {
    if first {
        TerminalPairMatchError::FirstFootprint(id)
    } else {
        TerminalPairMatchError::SecondFootprint(id)
    }
}

fn physical_error(
    first: bool,
    id: omega_selected_instructions::SelectedInstructionId,
) -> TerminalPairMatchError {
    if first {
        TerminalPairMatchError::FirstPhysicalSource(id)
    } else {
        TerminalPairMatchError::SecondPhysicalSource(id)
    }
}
