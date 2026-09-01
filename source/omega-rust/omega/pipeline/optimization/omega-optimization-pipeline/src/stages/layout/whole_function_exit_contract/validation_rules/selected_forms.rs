use std::collections::{BTreeMap, BTreeSet};

use omega_machine_optimizer::{PhysicalOperandFootprint, PostAllocationMachineInstruction};
use omega_register_model::{RegisterOperandAccess, RegisterUnitId, RegisterViewId};
use omega_selected_instructions::{
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionKind,
};
use omega_target::{Architecture, NativeTarget};
use psi_core::EdgeId;

use crate::{
    SelectedFormEncodingState, SelectedFormMachineDisposition,
    StagedOptimizedResolvedSelectedFormLayout, StagedOptimizedSelectedFormEncoding,
};

use super::super::{
    error::WholeFunctionExitContractError,
    model::{
        WholeFunctionReturnEvidence, WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
    },
};

pub(in crate::stages::layout::whole_function_exit_contract) fn unique_encoding_rows(
    encoding: &StagedOptimizedSelectedFormEncoding,
) -> Result<
    BTreeMap<SelectedInstructionId, &crate::SelectedFormEncodingRow>,
    WholeFunctionExitContractError,
> {
    let mut rows = BTreeMap::new();
    for row in encoding.rows() {
        if rows.insert(row.instruction, row).is_some() {
            return Err(WholeFunctionExitContractError::DuplicateInstruction(
                row.instruction,
            ));
        }
    }
    Ok(rows)
}

pub(in crate::stages::layout::whole_function_exit_contract) fn unique_layout_rows(
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<
    BTreeMap<
        SelectedInstructionId,
        (
            &crate::ResolvedSelectedBlockLayout,
            &crate::ResolvedSelectedFormRow,
        ),
    >,
    WholeFunctionExitContractError,
> {
    let mut rows = BTreeMap::new();
    for function in layout.functions() {
        for block in &function.blocks {
            for row in &block.instructions {
                if rows.insert(row.instruction, (block, row)).is_some() {
                    return Err(WholeFunctionExitContractError::DuplicateInstruction(
                        row.instruction,
                    ));
                }
            }
        }
    }
    Ok(rows)
}

pub(in crate::stages::layout::whole_function_exit_contract) fn reject_preservation_writes(
    machine: &PostAllocationMachineInstruction,
    callee_saved: &BTreeSet<RegisterUnitId>,
    link_units: &BTreeSet<RegisterUnitId>,
    instruction: SelectedInstructionId,
) -> Result<(), WholeFunctionExitContractError> {
    for unit in machine.unit_defs.iter().chain(&machine.unit_clobbers) {
        if callee_saved.contains(unit) {
            return Err(WholeFunctionExitContractError::CalleeSavedWrite {
                instruction,
                unit: *unit,
            });
        }
        if link_units.contains(unit) {
            return Err(WholeFunctionExitContractError::LinkRegisterWrite(
                instruction,
            ));
        }
    }
    Ok(())
}

pub(in crate::stages::layout::whole_function_exit_contract) fn reject_transformed_preservation_writes(
    encoding: &crate::SelectedFormEncodingRow,
    callee_saved: &BTreeSet<RegisterUnitId>,
    link_units: &BTreeSet<RegisterUnitId>,
    instruction: SelectedInstructionId,
) -> Result<(), WholeFunctionExitContractError> {
    let SelectedFormEncodingState::Encoded { footprint, .. } = &encoding.state else {
        return Ok(());
    };
    reject_implicit_unit_writes(
        &footprint.implicit_defs,
        &footprint.implicit_clobbers,
        callee_saved,
        link_units,
        instruction,
    )
}

fn reject_implicit_unit_writes(
    implicit_defs: &[RegisterUnitId],
    implicit_clobbers: &[RegisterUnitId],
    callee_saved: &BTreeSet<RegisterUnitId>,
    link_units: &BTreeSet<RegisterUnitId>,
    instruction: SelectedInstructionId,
) -> Result<(), WholeFunctionExitContractError> {
    for unit in implicit_defs.iter().chain(implicit_clobbers) {
        if callee_saved.contains(unit) {
            return Err(WholeFunctionExitContractError::CalleeSavedWrite {
                instruction,
                unit: *unit,
            });
        }
        if link_units.contains(unit) {
            return Err(WholeFunctionExitContractError::LinkRegisterWrite(
                instruction,
            ));
        }
    }
    Ok(())
}

pub(in crate::stages::layout::whole_function_exit_contract) fn transformed_implicit_writes_any(
    encoding: &crate::SelectedFormEncodingRow,
    units: &BTreeSet<RegisterUnitId>,
) -> bool {
    match &encoding.state {
        SelectedFormEncodingState::Encoded { footprint, .. } => footprint
            .implicit_defs
            .iter()
            .chain(&footprint.implicit_clobbers)
            .any(|unit| units.contains(unit)),
        SelectedFormEncodingState::DeferredControl { .. } => false,
    }
}

pub(in crate::stages::layout::whole_function_exit_contract) fn validate_non_return(
    instruction: SelectedInstructionId,
    conditional_terminator: bool,
    encoding: &crate::SelectedFormEncodingRow,
    layout: &crate::ResolvedSelectedFormRow,
) -> Result<(), WholeFunctionExitContractError> {
    let effects = match &encoding.state {
        SelectedFormEncodingState::Encoded { footprint, bytes } => {
            let disposition_matches = match encoding.machine_disposition {
                SelectedFormMachineDisposition::RetainedV1 => bytes == &layout.bytes,
                SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 { .. }
                | SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 { .. } => {
                    layout.bytes.is_empty()
                }
                SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 { .. } => false,
            };
            if conditional_terminator || !disposition_matches || layout.branch.is_some() {
                return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
                    instruction,
                ));
            }
            &footprint.encoded
        }
        SelectedFormEncodingState::DeferredControl { .. } => {
            if !conditional_terminator {
                return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
                    instruction,
                ));
            }
            layout
                .branch
                .as_ref()
                .map(|branch| &branch.decoded_effects)
                .ok_or(WholeFunctionExitContractError::InstructionRosterMismatch(
                    instruction,
                ))?
        }
    };
    if effects.stack != MachineEncodedStackEffect::UnchangedV1 {
        return Err(WholeFunctionExitContractError::NonReturnStackEffect(
            instruction,
        ));
    }
    if effects.memory != MachineEncodedMemoryEffect::NoneV1 {
        return Err(WholeFunctionExitContractError::NonReturnMemoryEffect(
            instruction,
        ));
    }
    let expected_control = if conditional_terminator {
        MachineEncodedControlEffect::ConditionalRelativeBranchV1
    } else {
        MachineEncodedControlEffect::FallThroughV1
    };
    if effects.control != expected_control {
        return Err(WholeFunctionExitContractError::NonReturnControlEffect(
            instruction,
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::stages::layout::whole_function_exit_contract) fn validate_return(
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    link_register: Option<RegisterViewId>,
    result_view: Option<RegisterViewId>,
    block: SelectedBlockId,
    psi_return_edge: EdgeId,
    selected: &omega_selected_instructions::SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    encoding: &crate::SelectedFormEncodingRow,
    layout: &crate::ResolvedSelectedFormRow,
    layout_block_end: u64,
) -> Result<WholeFunctionReturnEvidence, WholeFunctionExitContractError> {
    let value = match selected.kind {
        SelectedInstructionKind::ReturnI64 => {
            let Some(result_view) = result_view else {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            };
            let [operand]: &[PhysicalOperandFootprint] = machine.operands.as_slice() else {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            };
            if selected.operands.len() != 1
                || operand.operand != 0
                || operand.access != RegisterOperandAccess::Use
                || operand.view != result_view
                || operand.read_units != operand.storage_units
                || !operand.write_units.is_empty()
            {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            }
            WholeFunctionReturnValueEvidence::ScalarI64V1 {
                virtual_register: operand.virtual_register,
                view: operand.view,
                units: operand.storage_units.clone(),
            }
        }
        SelectedInstructionKind::ReturnUnit => {
            if !selected.operands.is_empty() || !machine.operands.is_empty() {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            }
            WholeFunctionReturnValueEvidence::UnitV1
        }
        _ => {
            return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                selected.id,
            ));
        }
    };
    let (bytes, effects): (&[u8], &MachineEncodedEffects) = match &encoding.state {
        SelectedFormEncodingState::Encoded { bytes, footprint } => (bytes, &footprint.encoded),
        SelectedFormEncodingState::DeferredControl { .. } => {
            return Err(WholeFunctionExitContractError::ReturnEncodingMismatch(
                selected.id,
            ));
        }
    };
    if bytes != layout.bytes || layout.branch.is_some() || effects != &machine.alternative.encoded {
        return Err(WholeFunctionExitContractError::ReturnEncodingMismatch(
            selected.id,
        ));
    }
    let end = layout
        .offset
        .checked_add(
            u64::try_from(layout.bytes.len())
                .map_err(|_| WholeFunctionExitContractError::OffsetOverflow)?,
        )
        .ok_or(WholeFunctionExitContractError::OffsetOverflow)?;
    if end != layout_block_end {
        return Err(WholeFunctionExitContractError::ReturnPlacementMismatch(
            selected.id,
        ));
    }
    let mechanism = match target.architecture {
        Architecture::X86_64 => {
            if effects.memory
                != (MachineEncodedMemoryEffect::ReadActivationStackV1 {
                    stack_pointer,
                    byte_count: 8,
                })
                || effects.stack
                    != (MachineEncodedStackEffect::PopBytesV1 {
                        stack_pointer,
                        byte_count: 8,
                    })
                || effects.control != MachineEncodedControlEffect::ReturnFromActivationStackV1
                || bytes != [0xc3]
            {
                return Err(WholeFunctionExitContractError::ReturnEffectsMismatch(
                    selected.id,
                ));
            }
            WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                stack_pointer,
                read_bytes: 8,
                pop_bytes: 8,
            }
        }
        Architecture::Aarch64 => {
            let link_register = link_register.ok_or(
                WholeFunctionExitContractError::ReturnEffectsMismatch(selected.id),
            )?;
            if effects.memory != MachineEncodedMemoryEffect::NoneV1
                || effects.stack != MachineEncodedStackEffect::UnchangedV1
                || effects.control
                    != (MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                        target: link_register,
                    })
                || bytes != [0xc0, 0x03, 0x5f, 0xd6]
            {
                return Err(WholeFunctionExitContractError::ReturnEffectsMismatch(
                    selected.id,
                ));
            }
            WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
                stack_pointer,
                link_register,
            }
        }
    };
    if effects.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1 {
        return Err(WholeFunctionExitContractError::ReturnEffectsMismatch(
            selected.id,
        ));
    }
    Ok(WholeFunctionReturnEvidence {
        block,
        psi_return_edge,
        instruction: selected.id,
        offset: layout.offset,
        bytes: layout.bytes.clone(),
        value,
        trap: effects.trap,
        mechanism,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use omega_register_model::RegisterUnitId;
    use omega_selected_instructions::SelectedInstructionId;

    use super::reject_implicit_unit_writes;

    #[test]
    fn transformed_rflags_clobbers_pass_when_not_abi_preserved() {
        let rflags = RegisterUnitId(91);
        assert!(
            reject_implicit_unit_writes(
                &[],
                &[rflags],
                &BTreeSet::new(),
                &BTreeSet::new(),
                SelectedInstructionId(7),
            )
            .is_ok()
        );
    }

    #[test]
    fn transformed_implicit_writes_cannot_bypass_abi_preservation() {
        let preserved = RegisterUnitId(13);
        assert!(
            reject_implicit_unit_writes(
                &[],
                &[preserved],
                &BTreeSet::from([preserved]),
                &BTreeSet::new(),
                SelectedInstructionId(8),
            )
            .is_err()
        );
    }
}
