//! Independently indexed replay of original-victim role eligibility.

use std::collections::BTreeMap;

use optimization_unit::ValueDefinitionSite;
use register_model::RegisterOperandAccess;
use selected_instructions::{SelectedInstruction, SelectedTerminator, VirtualRegisterOrigin};
use semantic_vocabulary::{IntegerCarrier, IntegerSign, ScalarType};

use crate::{
    GeneralizedSpillRecoveryChoiceError, GeneralizedSpillRecoveryResident, LiveRangePoint,
    VirtualFixedConstraintSite,
};

use super::{checked, to_u64};

pub(super) fn replay(
    register: selected_instructions::VirtualRegisterId,
    block: selected_instructions::SelectedBlockId,
    point: LiveRangePoint,
    resident: GeneralizedSpillRecoveryResident,
    selected: &selected_instructions::SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    steps: &mut u64,
) -> Result<bool, GeneralizedSpillRecoveryChoiceError> {
    *steps = checked(*steps, to_u64(selected.virtual_registers.len())?)?;
    *steps = checked(*steps, to_u64(ranges.virtual_registers.len())?)?;
    let selected_values = selected
        .virtual_registers
        .iter()
        .map(|value| (value.id, value))
        .collect::<BTreeMap<_, _>>();
    let range_values = ranges
        .virtual_registers
        .iter()
        .map(|range| (range.virtual_register, range))
        .collect::<BTreeMap<_, _>>();
    let Some(value) = selected_values.get(&register).copied() else {
        return Ok(false);
    };
    let Some(range) = range_values.get(&register).copied() else {
        return Ok(false);
    };
    *steps = checked(*steps, to_u64(range.occurrences.len())?)?;
    *steps = checked(*steps, to_u64(range.fixed_constraints.len())?)?;
    let Some(selected_block) = selected.blocks.iter().find(|row| row.id == block) else {
        return Ok(false);
    };
    *steps = checked(
        *steps,
        to_u64(selected_block.instructions.len().saturating_add(1))?,
    )?;
    let instructions = selected_block
        .instructions
        .iter()
        .chain(std::iter::once(match &selected_block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .map(|instruction| (instruction.id, instruction))
        .collect::<BTreeMap<_, _>>();
    let scalar_ok = matches!(
        value.scalar_type,
        ScalarType::Integer(integer)
            if integer.carrier() == IntegerCarrier::Fixed
                && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64
    );
    let VirtualRegisterOrigin::InstructionResult {
        instruction: definition,
        ..
    } = value.origin
    else {
        return Ok(false);
    };
    let definition_site_ok = matches!(
        value.definition_site,
        ValueDefinitionSite::Node { block: source, .. } if source == selected_block.source_block
    );
    if !scalar_ok
        || !definition_site_ok
        || value.class != resident.class
        || ranges.machine != selected.machine
        || range.class != resident.class
        || range.fragments.as_slice()
            != [crate::LiveRangeFragment {
                block,
                start: resident.start,
                end: resident.exclusive_end,
            }]
        || !range.edge_connectors.is_empty()
        || resident.start >= point
        || resident.exclusive_end <= point
        || range
            .occurrences
            .iter()
            .any(|occurrence| occurrence.point == point)
        || range.fixed_constraints.iter().any(|fixed| {
            matches!(fixed.site, VirtualFixedConstraintSite::Operand { point: fixed_point, .. } if fixed_point > point)
        })
    {
        return Ok(false);
    }
    let definitions = range
        .occurrences
        .iter()
        .filter(|occurrence| {
            occurrence.instruction == definition
                && occurrence.access == RegisterOperandAccess::Def
                && occurrence.point < point
        })
        .count();
    if definitions != 1
        || !instructions.get(&definition).is_some_and(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.virtual_register == register
                    && operand.access == RegisterOperandAccess::Def
                    && operand.class == value.class
            })
        })
    {
        return Ok(false);
    }
    let later = range
        .occurrences
        .iter()
        .filter(|occurrence| occurrence.point > point)
        .collect::<Vec<_>>();
    Ok(!later.is_empty()
        && later.iter().all(|occurrence| {
            instructions.get(&occurrence.instruction).is_some_and(
                |instruction: &&SelectedInstruction| {
                    instruction.operands.iter().any(|operand| {
                        operand.operand == occurrence.operand
                            && occurrence.access == RegisterOperandAccess::Use
                            && operand.virtual_register == register
                            && operand.access == RegisterOperandAccess::Use
                            && operand.class == value.class
                            && operand.fixed_view.is_none()
                            && operand.tied_to.is_none()
                            && !operand.early_clobber
                    })
                },
            )
        }))
}
