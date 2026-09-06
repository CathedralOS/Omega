use std::collections::BTreeSet;

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::InstructionLiveness;
use selected_instructions::{MachineSemanticKind, SelectedInstruction, SelectedInstructionKind};

use physical_instructions::{PhysicalOperandFootprint, PostAllocationMachineInstruction};

use super::{
    ControlPattern, FixedViewPattern, InstructionPairMatchError, InstructionPattern,
    MatchedPhysicalRead, OperandReadPattern, OperandWritePattern, ViewPattern,
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
) -> Result<Vec<MatchedPhysicalRead>, InstructionPairMatchError> {
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
        || !control_matches(pattern.control, encoded.control, physical)
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

fn control_matches(
    pattern: ControlPattern,
    actual: selected_instructions::MachineEncodedControlEffect,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    match pattern {
        ControlPattern::Exact(expected) => actual == expected,
        ControlPattern::ReturnIndirectNamed(name) => physical
            .model()
            .view_named(name)
            .is_some_and(|view| {
                actual
                    == selected_instructions::MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                        target: view.id,
                    }
            }),
    }
}

fn match_operand(
    pattern: &super::model::OperandPattern,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    first: bool,
) -> Result<MatchedPhysicalRead, InstructionPairMatchError> {
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
        || selected_operand.tied_to != pattern.tied_to
        || selected_operand.early_clobber != pattern.early_clobber
    {
        return Err(footprint_error(first, selected.id));
    }
    let view = match_view(pattern.view, operand, physical)
        .map_err(|()| physical_error(first, selected.id))?;
    let read_matches = match pattern.read {
        OperandReadPattern::Empty => operand.read_units.is_empty(),
        OperandReadPattern::StorageUnits => operand.read_units == operand.storage_units,
    };
    let write_matches = match pattern.write {
        OperandWritePattern::Empty => {
            operand.write_units.is_empty() && operand.write_semantics.is_none()
        }
        OperandWritePattern::ViewWrite => {
            operand.write_units == view.write_units
                && operand.write_semantics == Some(view.write_semantics)
        }
    };
    if !read_matches || !write_matches {
        return Err(footprint_error(first, selected.id));
    }
    let fixed_view_matches = match pattern.fixed_view {
        FixedViewPattern::None => selected_operand.fixed_view.is_none(),
        FixedViewPattern::Named(name) => physical
            .model()
            .view_named(name)
            .is_some_and(|fixed| selected_operand.fixed_view == Some(fixed.id)),
    };
    if !fixed_view_matches {
        return Err(footprint_error(first, selected.id));
    }
    Ok(MatchedPhysicalRead {
        source_instruction: selected.id,
        operand: operand.operand,
        virtual_register: operand.virtual_register,
        class: operand.class,
        view: operand.view,
        storage_units: operand.storage_units.clone(),
        units: operand.read_units.clone(),
        write_units: operand.write_units.clone(),
    })
}

fn match_view<'a>(
    pattern: ViewPattern,
    operand: &PhysicalOperandFootprint,
    physical: &'a ValidatedPhysicalRegisterModel,
) -> Result<&'a register_model::RegisterView, ()> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == operand.view)
        .ok_or(())?;
    let matches = match pattern {
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
            !(!valid_index
                || view.bits != bits
                || !view.allocatable
                || view.class != operand.class
                || view.units != operand.storage_units)
        }
        ViewPattern::Named { name, bits } => {
            view.name == name
                && view.bits == bits
                && view.allocatable
                && view.class == operand.class
                && view.units == operand.storage_units
        }
    };
    matches.then_some(view).ok_or(())
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
        SelectedInstructionKind::CompareI64 => MachineSemanticKind::CompareI64,
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
        SelectedInstructionKind::ConditionalBranchU64LessThan => {
            MachineSemanticKind::ConditionalBranchU64LessThan
        }
        SelectedInstructionKind::ConditionalBranchI64LessThan => {
            MachineSemanticKind::ConditionalBranchI64LessThan
        }
        SelectedInstructionKind::Jump => MachineSemanticKind::Jump,
        SelectedInstructionKind::ReturnI64 => MachineSemanticKind::ReturnI64,
        SelectedInstructionKind::ReturnUnit => MachineSemanticKind::ReturnUnit,
        SelectedInstructionKind::CallI64 { .. } => {
            unreachable!("scalar calls are refused before machine optimization")
        }
    }
}

fn roster_error(
    first: bool,
    id: selected_instructions::SelectedInstructionId,
) -> InstructionPairMatchError {
    if first {
        InstructionPairMatchError::FirstRoster(id)
    } else {
        InstructionPairMatchError::SecondRoster(id)
    }
}

fn footprint_error(
    first: bool,
    id: selected_instructions::SelectedInstructionId,
) -> InstructionPairMatchError {
    if first {
        InstructionPairMatchError::FirstFootprint(id)
    } else {
        InstructionPairMatchError::SecondFootprint(id)
    }
}

fn physical_error(
    first: bool,
    id: selected_instructions::SelectedInstructionId,
) -> InstructionPairMatchError {
    if first {
        InstructionPairMatchError::FirstPhysicalSource(id)
    } else {
        InstructionPairMatchError::SecondPhysicalSource(id)
    }
}
