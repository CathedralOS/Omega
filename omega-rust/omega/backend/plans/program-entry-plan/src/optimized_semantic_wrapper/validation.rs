//! Independent replay of wrapper source custody, geometry, and action order.

use calling_conventions::{
    CallingPolicy, IndirectPointerLocation, MachineRegister, ValueLocation, ValuePlacement,
};

use crate::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    ProgramEntrySourceReceiverSignature, ProgramStorageEntryDiagnostic,
    ProgramStorageEntryRootRole,
};

use super::model::{
    OptimizedProgramStorageSemanticReceiverStorage,
    OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    OptimizedProgramStorageSemanticWrapperPlan, OptimizedProgramStorageSemanticWrapperStep,
};
use super::recipe::{
    EXTENT_ALIGNMENT, EXTENT_BYTE_COUNT, OUTGOING_FRAME_BYTE_COUNT,
    OptimizedProgramStorageSemanticReceiverLayout, PRE_CALL_STACK_ALIGNMENT,
    RECEIVER_SLOT_BYTE_OFFSET, SHADOW_BYTE_COUNT, expected_relocation, expected_steps,
};

/// The receiver residence is not a boundary input: nothing outside the
/// wrapper can supply it. Its checked extent is a property of the emitted
/// semantic child, so the caller derives the referent layout there and the
/// plan records it verbatim for downstream custody.
pub(super) fn receiver_storage(
    receiver: Option<OptimizedProgramStorageSemanticReceiverLayout>,
    contract: &OptimizedProgramStorageSemanticEntryContract,
) -> Result<Option<OptimizedProgramStorageSemanticReceiverStorage>, ProgramStorageEntryDiagnostic> {
    let declared = matches!(
        contract.source_signature().receiver(),
        ProgramEntrySourceReceiverSignature::ProvisionedMutable { .. }
    );
    match (receiver, declared) {
        (None, false) => Ok(None),
        (Some(_), false) | (None, true) => Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper receiver storage must match the exact receiver signature"
                .into(),
        )),
        (Some(receiver), true) => {
            if receiver.byte_count() == 0
                || !receiver.alignment().is_power_of_two()
                || receiver.alignment() > 16
                || receiver.byte_count() > u16::MAX as u32
            {
                return Err(ProgramStorageEntryDiagnostic(
                    "optimized semantic ProgramStorage wrapper receiver requires a checked layout within one aligned slot"
                        .into(),
                ));
            }
            let slot_byte_count = receiver.slot_byte_count().ok_or(ProgramStorageEntryDiagnostic(
                "optimized semantic ProgramStorage wrapper receiver layout overflowed its slot"
                    .into(),
            ))?;
            Ok(Some(OptimizedProgramStorageSemanticReceiverStorage {
                byte_count: receiver.byte_count(),
                alignment: receiver.alignment(),
                slot_byte_count,
                outgoing_stack_byte_offset: RECEIVER_SLOT_BYTE_OFFSET,
            }))
        }
    }
}

pub(super) fn validate(
    plan: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    validate_contract_surface(&plan.source)?;
    let expected_frame = match plan.receiver {
        Some(receiver) => receiver
            .outgoing_stack_byte_offset
            .checked_add(receiver.slot_byte_count)
            .and_then(|end| end.checked_add(8))
            .ok_or(ProgramStorageEntryDiagnostic(
                "optimized semantic ProgramStorage wrapper receiver frame overflowed".into(),
            ))?,
        None => OUTGOING_FRAME_BYTE_COUNT,
    };
    if plan.source_signature_identity != plan.source.source_signature_identity()
        || plan.shadow_byte_count != SHADOW_BYTE_COUNT
        || plan.outgoing_frame_byte_count != expected_frame
        || plan.outgoing_release_byte_count != plan.outgoing_frame_byte_count
        || plan.pre_call_stack_alignment != PRE_CALL_STACK_ALIGNMENT
        || (plan.receiver.is_some()
            != matches!(
                plan.source.source_signature().receiver(),
                ProgramEntrySourceReceiverSignature::ProvisionedMutable { .. }
            ))
        || plan.encoding_disposition
            != OptimizedProgramStorageSemanticWrapperEncodingDisposition::TargetEncodingRequiredV1
        || plan.physical_disposition
            != OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper frame or source custody drifted".into(),
        ));
    }
    if let Some(receiver) = plan.receiver {
        if receiver.outgoing_stack_byte_offset != RECEIVER_SLOT_BYTE_OFFSET
            || receiver.slot_byte_count & 15 != 0
            || receiver.slot_byte_count < receiver.byte_count
            || receiver.slot_byte_count < 16
            || receiver.byte_count == 0
            || !receiver.alignment.is_power_of_two()
            || receiver.alignment > 16
        {
            return Err(ProgramStorageEntryDiagnostic(
                "optimized semantic ProgramStorage wrapper receiver residence drifted".into(),
            ));
        }
    }
    replay_steps(plan)?;
    let call_step_index = plan
        .steps
        .iter()
        .position(|step| {
            matches!(
                step,
                OptimizedProgramStorageSemanticWrapperStep::CallPrivateTerminalContinuation { .. }
            )
        })
        .ok_or(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper has no continuation call".into(),
        ))?;
    if plan.relocation != expected_relocation(call_step_index) {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper call relocation drifted".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_contract_surface(
    contract: &OptimizedProgramStorageSemanticEntryContract,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if contract.target() != target::NativeTarget::uefi_x64()
        || contract.semantic_boundary_entry_plan().call.policy != CallingPolicy::MicrosoftX64
        || contract
            .semantic_boundary_entry_plan()
            .call
            .result
            .is_some()
        || contract.physical_disposition()
            != OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper requires one non-invoked UEFI Microsoft-x64 Unit contract"
                .into(),
        ));
    }
    let [image, storage] = contract.roots();
    validate_root_placement(
        image.role(),
        image.parameter_index(),
        image.placement(),
        ProgramStorageEntryRootRole::Image,
        0,
        MachineRegister::X86Rcx,
        32,
    )?;
    validate_root_placement(
        storage.role(),
        storage.parameter_index(),
        storage.placement(),
        ProgramStorageEntryRootRole::InitialStorage,
        1,
        MachineRegister::X86Rdx,
        48,
    )
}

fn validate_root_placement(
    actual_role: ProgramStorageEntryRootRole,
    actual_index: usize,
    placement: &ValuePlacement,
    expected_role: ProgramStorageEntryRootRole,
    expected_index: usize,
    expected_register: MachineRegister,
    expected_copy_offset: u32,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if actual_role != expected_role
        || actual_index != expected_index
        || placement.shape.byte_size != EXTENT_BYTE_COUNT
        || placement.shape.alignment != EXTENT_ALIGNMENT
        || !matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: Some(copy_offset),
                byte_size: EXTENT_BYTE_COUNT,
                alignment: EXTENT_ALIGNMENT,
            }] if *register == expected_register && *copy_offset == expected_copy_offset
        )
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "optimized semantic ProgramStorage {expected_role:?} placement drifted"
        )));
    }
    Ok(())
}

/// The replay is the whole recipe — the incoming boundary plan never carries
/// a receiver, so when the source signature provisions one the wrapper itself
/// owns the zeroed residence and shifts the continuation's argument order to
/// `self`, image, storage.
fn replay_steps(
    plan: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let fingerprint = plan.source.semantic_calling_plan_report_fingerprint();
    if plan.steps != expected_steps(fingerprint, plan.receiver) {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper action sequence drifted".into(),
        ));
    }
    Ok(())
}
