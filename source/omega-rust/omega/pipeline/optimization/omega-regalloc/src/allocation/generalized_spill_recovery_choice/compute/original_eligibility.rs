//! Exact selected-role eligibility for an original epoch-two victim.

use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::RegisterOperandAccess;
use omega_selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedTerminator, VirtualRegisterOrigin,
};
use psi_core::{IntegerCarrier, IntegerSign, ScalarType};

use crate::{
    GeneralizedSpillRecoveryChoiceError, GeneralizedSpillRecoveryResident, LiveRangePoint,
    VirtualFixedConstraintSite,
};

use super::{Work, add, count};

#[allow(clippy::too_many_arguments)]
pub(super) fn is_eligible(
    register: omega_selected_instructions::VirtualRegisterId,
    block: omega_selected_instructions::SelectedBlockId,
    point: LiveRangePoint,
    resident: &GeneralizedSpillRecoveryResident,
    selected: &SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    work: &mut Work,
) -> Result<bool, GeneralizedSpillRecoveryChoiceError> {
    add(&mut work.steps, count(selected.virtual_registers.len())?)?;
    add(&mut work.steps, count(ranges.virtual_registers.len())?)?;
    let Some(value) = selected
        .virtual_registers
        .iter()
        .find(|value| value.id == register)
    else {
        return Ok(false);
    };
    let Some(range) = ranges
        .virtual_registers
        .iter()
        .find(|range| range.virtual_register == register)
    else {
        return Ok(false);
    };
    add(&mut work.steps, count(range.occurrences.len())?)?;
    add(&mut work.steps, count(range.fixed_constraints.len())?)?;
    let Some(selected_block) = selected.blocks.iter().find(|row| row.id == block) else {
        return Ok(false);
    };
    add(
        &mut work.steps,
        count(selected_block.instructions.len().saturating_add(1))?,
    )?;
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
        || !instruction(selected_block, definition).is_some_and(|instruction| {
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
            instruction(selected_block, occurrence.instruction).is_some_and(|instruction| {
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
            })
        }))
}

fn instruction(
    block: &omega_selected_instructions::SelectedBlock,
    id: omega_selected_instructions::SelectedInstructionId,
) -> Option<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .find(|instruction| instruction.id == id)
}
