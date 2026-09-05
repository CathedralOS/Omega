//! Exact permissions supplied by one validated ordinary fixed frame.

use std::collections::BTreeSet;

use omega_register_model::{RegisterUnitId, RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionId,
};
use omega_target::{Architecture, NativeTarget};

use omega_machine_code::ResolvedSelectedFormRow;
use omega_post_allocation_machine_to_frame_layout::{
    FunctionTargetFrameLayout, ReturnAddressFrameCustody,
};
use omega_post_allocation_machine_to_selected_form_encoding::{
    SelectedFormEncodingRow, SelectedFormEncodingState, SelectedFormMachineDisposition,
};

use super::super::WholeFunctionExitContractError;

pub(in crate::exit_contract) fn frame_permissions(
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&FunctionTargetFrameLayout>,
) -> Result<(BTreeSet<RegisterUnitId>, bool), WholeFunctionExitContractError> {
    let Some(frame) = frame else {
        return Ok((BTreeSet::new(), false));
    };
    let mut units = BTreeSet::new();
    for slot in &frame.callee_save_slots {
        let view = physical
            .model()
            .views
            .iter()
            .find(|view| view.id == slot.storage_view)
            .ok_or(WholeFunctionExitContractError::InvalidConvention)?;
        units.extend(view.units.iter().copied());
    }
    Ok((
        units,
        matches!(
            frame.return_address,
            ReturnAddressFrameCustody::SavedLinkRegister { .. }
        ),
    ))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::exit_contract) fn validate_preservation_writes(
    machine: &omega_machine_optimizer::PostAllocationMachineInstruction,
    encoding: &SelectedFormEncodingRow,
    callee_saved: &BTreeSet<RegisterUnitId>,
    link_units: &BTreeSet<RegisterUnitId>,
    allowed_callee_saved: &BTreeSet<RegisterUnitId>,
    allow_link_write: bool,
    instruction: SelectedInstructionId,
    modified_callee_saved: &mut BTreeSet<RegisterUnitId>,
) -> Result<(), WholeFunctionExitContractError> {
    let mut validate = |unit: &RegisterUnitId| {
        if callee_saved.contains(unit) {
            if !allowed_callee_saved.contains(unit) {
                return Err(WholeFunctionExitContractError::CalleeSavedWrite {
                    instruction,
                    unit: *unit,
                });
            }
            modified_callee_saved.insert(*unit);
        }
        if link_units.contains(unit) && !allow_link_write {
            return Err(WholeFunctionExitContractError::LinkRegisterWrite(
                instruction,
            ));
        }
        Ok(())
    };
    for unit in machine.unit_defs.iter().chain(&machine.unit_clobbers) {
        validate(unit)?;
    }
    let footprint = match &encoding.state {
        SelectedFormEncodingState::Encoded { footprint, .. }
        | SelectedFormEncodingState::UnresolvedInternalMachineCall { footprint, .. } => {
            Some(footprint)
        }
        SelectedFormEncodingState::DeferredControl { .. } => None,
    };
    if let Some(footprint) = footprint {
        for unit in footprint
            .implicit_defs
            .iter()
            .chain(&footprint.implicit_clobbers)
        {
            validate(unit)?;
        }
    }
    Ok(())
}

pub(in crate::exit_contract) fn validate_internal_call(
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    instruction: SelectedInstructionId,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
) -> Result<(), WholeFunctionExitContractError> {
    let SelectedFormEncodingState::UnresolvedInternalMachineCall {
        bytes,
        footprint,
        fixup,
    } = &encoding.state
    else {
        return Err(WholeFunctionExitContractError::NonReturnControlEffect(
            instruction,
        ));
    };
    if encoding.machine_disposition != SelectedFormMachineDisposition::RetainedV1
        || bytes != &layout.bytes
        || layout.branch.is_some()
        || layout.internal_machine_fixup != Some(*fixup)
        || footprint.encoded.control != MachineEncodedControlEffect::DirectRelativeCallV1
        || footprint.encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
    {
        return Err(WholeFunctionExitContractError::NonReturnControlEffect(
            instruction,
        ));
    }
    let effects_match = match target.architecture {
        Architecture::X86_64 => {
            footprint.encoded.memory
                == MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                    stack_pointer,
                    byte_count: 8,
                }
                && footprint.encoded.stack
                    == MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                        stack_pointer,
                        return_address_byte_count: 8,
                    }
        }
        Architecture::Aarch64 => {
            footprint.encoded.memory == MachineEncodedMemoryEffect::NoneV1
                && footprint.encoded.stack == MachineEncodedStackEffect::UnchangedV1
        }
    };
    if !effects_match {
        return Err(WholeFunctionExitContractError::NonReturnStackEffect(
            instruction,
        ));
    }
    Ok(())
}
