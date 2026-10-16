//! Exact permissions supplied by one validated ordinary fixed frame.

use std::collections::BTreeSet;

use register_model::{RegisterUnitId, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedCallTarget, SelectedInstructionId,
};
use target::{Architecture, NativeTarget};

use crate::frame_layout::{FunctionTargetFrameLayout, ReturnAddressFrameCustody};
use machine_code::ResolvedSelectedFormRow;
use machine_code::{
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
            .view(slot.storage_view)
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
    machine: &physical_instructions::PostAllocationMachineInstruction,
    encoding: &SelectedFormEncodingRow,
    callee_saved: &BTreeSet<RegisterUnitId>,
    link_units: &BTreeSet<RegisterUnitId>,
    allowed_callee_saved: &BTreeSet<RegisterUnitId>,
    allow_link_write: bool,
    instruction: SelectedInstructionId,
    modified_callee_saved: &mut BTreeSet<RegisterUnitId>,
    encoded: &machine_code::SelectedFormEncodedRowStore,
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
    // A deferred control row names no footprint, so the zero handle reads as
    // absent and contributes no implicit unit.
    if let Some(footprint) = encoded.footprint(encoding.footprint) {
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
    call_target: SelectedCallTarget,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), WholeFunctionExitContractError> {
    let Some((bytes, footprint)) = encoded.encoded(encoding) else {
        return Err(WholeFunctionExitContractError::NonReturnControlEffect(
            instruction,
        ));
    };
    // Direct calls retain a relocation field. Runtime table calls instead
    // already contain their register CALL/BLR and zero-slot trap, so their
    // returning path owns the same frame without an internal-machine fixup.
    let control_matches = match (call_target, &encoding.state) {
        (
            SelectedCallTarget::Machine(callee),
            SelectedFormEncodingState::UnresolvedInternalMachineCall { fixup },
        ) => {
            fixup.callee == callee
                && layout.internal_machine_fixup == Some(*fixup)
                && footprint.encoded.control == MachineEncodedControlEffect::DirectRelativeCallV1
        }
        (SelectedCallTarget::Dynamic { .. }, SelectedFormEncodingState::Encoded) => {
            layout.internal_machine_fixup.is_none()
                && footprint.encoded.control
                    == MachineEncodedControlEffect::IndirectTableCallOrTrapV1
        }
        _ => false,
    };
    if encoding.machine_disposition != SelectedFormMachineDisposition::RetainedV1
        || bytes != layout.bytes
        || layout.branch.is_some()
        || layout.normalized_foreign_call_fixup.is_some()
        || !control_matches
        || footprint.encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
    {
        return Err(WholeFunctionExitContractError::NonReturnControlEffect(
            instruction,
        ));
    }
    let effects_match = call_memory_stack_effects_match(target, stack_pointer, footprint);
    if !effects_match {
        return Err(WholeFunctionExitContractError::NonReturnStackEffect(
            instruction,
        ));
    }
    Ok(())
}

/// Imports carry an unresolved field and the ordinary return-address lifecycle.
/// Kernel entries instead retain fully decoded supervisor bytes, with no import
/// field or user-space stack lifecycle. The selected canonical constraint (not
/// the proposed encoding state) determines which contract must be checked.
pub(in crate::exit_contract) fn validate_normalized_foreign_call(
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    instruction: SelectedInstructionId,
    constraint: register_model::RegisterConstraintKey,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), WholeFunctionExitContractError> {
    if constraint.family == register_model::RegisterConstraintFamily::SystemCall {
        let canonical = if target == NativeTarget::linux_x64() {
            isa_x86_64::x86_64_linux_system_call_keys()
        } else if target == NativeTarget::linux_arm64() {
            isa_aarch64::aarch64_linux_system_call_keys()
        } else {
            return Err(WholeFunctionExitContractError::NonReturnControlEffect(
                instruction,
            ));
        };
        if !matches!(encoding.state, SelectedFormEncodingState::Encoded) {
            return Err(WholeFunctionExitContractError::NonReturnControlEffect(
                instruction,
            ));
        }
        let Some((bytes, footprint)) = encoded.encoded(encoding) else {
            return Err(WholeFunctionExitContractError::NonReturnControlEffect(
                instruction,
            ));
        };
        if !canonical.contains(&constraint)
            || encoding.machine_disposition != SelectedFormMachineDisposition::RetainedV1
            || encoding.address.is_some()
            || bytes != layout.bytes
            || layout.branch.is_some()
            || layout.internal_machine_fixup.is_some()
            || layout.normalized_foreign_call_fixup.is_some()
            || footprint.encoded.control != MachineEncodedControlEffect::SupervisorCallV1
            || footprint.encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        {
            return Err(WholeFunctionExitContractError::NonReturnControlEffect(
                instruction,
            ));
        }
        if footprint.encoded.memory != MachineEncodedMemoryEffect::NoneV1
            || footprint.encoded.stack != MachineEncodedStackEffect::UnchangedV1
        {
            return Err(WholeFunctionExitContractError::NonReturnStackEffect(
                instruction,
            ));
        }
        return Ok(());
    }
    let Some((bytes, footprint)) = encoded.encoded(encoding) else {
        return Err(WholeFunctionExitContractError::NonReturnControlEffect(
            instruction,
        ));
    };
    let control_matches = match &encoding.state {
        SelectedFormEncodingState::UnresolvedNormalizedForeignCall { fixup } => {
            layout.normalized_foreign_call_fixup.as_ref() == Some(fixup)
                && footprint.encoded.control == MachineEncodedControlEffect::DirectRelativeCallV1
        }
        SelectedFormEncodingState::Encoded => {
            layout.normalized_foreign_call_fixup.is_none()
                && footprint.encoded.control == MachineEncodedControlEffect::IndirectExternalCallV1
        }
        _ => false,
    };
    if encoding.machine_disposition != SelectedFormMachineDisposition::RetainedV1
        || bytes != layout.bytes
        || layout.branch.is_some()
        || layout.internal_machine_fixup.is_some()
        || !control_matches
        || footprint.encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
    {
        return Err(WholeFunctionExitContractError::NonReturnControlEffect(
            instruction,
        ));
    }
    let effects_match = call_memory_stack_effects_match(target, stack_pointer, footprint);
    if !effects_match {
        return Err(WholeFunctionExitContractError::NonReturnStackEffect(
            instruction,
        ));
    }
    Ok(())
}

fn call_memory_stack_effects_match(
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    footprint: &machine_code::SelectedFormDecodedFootprint,
) -> bool {
    let stack_matches = match target.architecture {
        Architecture::X86_64 => {
            footprint.encoded.stack
                == MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                    stack_pointer,
                    return_address_byte_count: 8,
                }
        }
        Architecture::Aarch64 => footprint.encoded.stack == MachineEncodedStackEffect::UnchangedV1,
    };
    let memory_matches = match footprint.encoded.control {
        MachineEncodedControlEffect::IndirectExternalCallV1 => {
            matches!(footprint.encoded.memory,
                MachineEncodedMemoryEffect::ReadTableFieldV1 { pointer_operand, .. }
                    if footprint.encoded.external_operand_reads.contains(&pointer_operand))
        }
        MachineEncodedControlEffect::IndirectTableCallOrTrapV1 => {
            matches!(footprint.encoded.memory,
                MachineEncodedMemoryEffect::ReadPointerV1 { pointer_operand, byte_count: 8 }
                    if footprint.encoded.external_operand_reads.last() == Some(&pointer_operand))
        }
        MachineEncodedControlEffect::DirectRelativeCallV1 => match target.architecture {
            Architecture::X86_64 => {
                footprint.encoded.memory
                    == MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                        stack_pointer,
                        byte_count: 8,
                    }
            }
            Architecture::Aarch64 => footprint.encoded.memory == MachineEncodedMemoryEffect::NoneV1,
        },
        _ => false,
    };
    stack_matches && memory_matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use machine_code::{SelectedFormDecodedFootprint, SelectedFormEncodingRow};
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind,
    };
    use semantic_vocabulary::BoundaryMachineId;

    #[test]
    fn system_call_exit_contract_preserves_supervisor_control_without_import_stack_effects() {
        let instruction = SelectedInstructionId(7);
        let kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: BoundaryMachineId::new(3).unwrap(),
            ordinal: 0,
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::NormalizedForeignCall,
            variant: 0,
        };
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let (bytes, footprint, constraint, stack_pointer) =
                if target == NativeTarget::linux_x64() {
                    let physical = isa_x86_64::validated_x86_64_physical_register_model();
                    let encoded = isa_x86_64::encode_x86_64_selected_system_call_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &[],
                        &[],
                        211,
                    )
                    .unwrap();
                    let footprint = encoded.footprint();
                    (
                        encoded.bytes().to_vec(),
                        SelectedFormDecodedFootprint {
                            register_reads: footprint.register_reads.clone(),
                            register_writes: footprint.register_writes.clone(),
                            implicit_defs: footprint.encoded.implicit_unit_defs.clone(),
                            implicit_clobbers: footprint.encoded.implicit_unit_clobbers.clone(),
                            encoded: footprint.encoded.clone(),
                        },
                        isa_x86_64::x86_64_linux_system_call_keys()[0],
                        physical.model().view_named("rsp").unwrap().id,
                    )
                } else {
                    let physical = isa_aarch64::validated_aarch64_physical_register_model();
                    let encoded = isa_aarch64::encode_aarch64_selected_system_call_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &[],
                        &[],
                        211,
                    )
                    .unwrap();
                    let footprint = encoded.footprint();
                    (
                        encoded.bytes().to_vec(),
                        SelectedFormDecodedFootprint {
                            register_reads: footprint.register_reads.clone(),
                            register_writes: footprint.register_writes.clone(),
                            implicit_defs: footprint.encoded.implicit_unit_defs.clone(),
                            implicit_clobbers: footprint.encoded.implicit_unit_clobbers.clone(),
                            encoded: footprint.encoded.clone(),
                        },
                        isa_aarch64::aarch64_linux_system_call_keys()[0],
                        physical.model().view_named("sp").unwrap().id,
                    )
                };
            let mut store = machine_code::SelectedFormEncodedRowStore::new();
            let (code, interned_footprint) = store.intern(&bytes, footprint);
            let (encoding, layout) = super::super::selected_forms::encoded_form_and_layout(
                instruction,
                alternative,
                code,
                interned_footprint,
                bytes,
            );
            let validate =
                |encoding: &SelectedFormEncodingRow,
                 layout: &ResolvedSelectedFormRow,
                 encoded: &machine_code::SelectedFormEncodedRowStore| {
                    validate_normalized_foreign_call(
                        target,
                        stack_pointer,
                        instruction,
                        constraint,
                        encoding,
                        layout,
                        encoded,
                    )
                };
            validate(&encoding, &layout, &store).unwrap();
            for mutation in 0..5 {
                // Both the bytes and the footprint a mutation forges live in
                // the store, so each mutation forges its own copy of it.
                let mut forged = store.clone();
                let footprint = forged.footprint_mut(encoding.footprint).unwrap();
                match mutation {
                    0 => {
                        footprint.encoded.control =
                            MachineEncodedControlEffect::DirectRelativeCallV1
                    }
                    1 => footprint.encoded.trap = MachineEncodedTrapBehavior::NeverV1,
                    2 => {
                        footprint.encoded.memory =
                            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                                stack_pointer,
                                byte_count: 8,
                            }
                    }
                    3 => {
                        footprint.encoded.stack =
                            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                                stack_pointer,
                                return_address_byte_count: 8,
                            }
                    }
                    _ => forged.bytes_mut(encoding.code)[0] ^= 1,
                }
                assert!(
                    validate(&encoding, &layout, &forged).is_err(),
                    "{target:?}, mutation {mutation}"
                );
            }
            let mut import_constraint = constraint;
            import_constraint.family = register_model::RegisterConstraintFamily::Call;
            assert!(
                validate_normalized_foreign_call(
                    target,
                    stack_pointer,
                    instruction,
                    import_constraint,
                    &encoding,
                    &layout,
                    &store,
                )
                .is_err()
            );
            assert!(
                validate_normalized_foreign_call(
                    NativeTarget::windows_x64(),
                    stack_pointer,
                    instruction,
                    constraint,
                    &encoding,
                    &layout,
                    &store,
                )
                .is_err()
            );
        }
    }
}
