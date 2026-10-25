//! Independent replay of wrapper source custody, geometry, and action order.

use calling_conventions::{
    CallingPolicy, IndirectPointerLocation, MachineRegister, ValueLocation, ValuePlacement,
};

use crate::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    ProgramStorageEntryDiagnostic, ProgramStorageEntryRootRole,
};

use super::recipe::{
    EXTENT_ALIGNMENT, EXTENT_BYTE_COUNT, OUTGOING_FRAME_BYTE_COUNT, PRE_CALL_STACK_ALIGNMENT,
    SHADOW_BYTE_COUNT, expected_relocation, expected_steps,
};
use super::{
    OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    OptimizedProgramStorageSemanticWrapperPlan, OptimizedProgramStorageSemanticWrapperStep,
};

pub(super) fn validate(
    plan: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    validate_contract_surface(&plan.source)?;
    let expected_frame = OUTGOING_FRAME_BYTE_COUNT;
    if plan.source_signature_identity != plan.source.source_signature_identity()
        || plan.shadow_byte_count != SHADOW_BYTE_COUNT
        || plan.outgoing_frame_byte_count != expected_frame
        || plan.outgoing_release_byte_count != plan.outgoing_frame_byte_count
        || plan.pre_call_stack_alignment != PRE_CALL_STACK_ALIGNMENT
        || plan.encoding_disposition
            != OptimizedProgramStorageSemanticWrapperEncodingDisposition::TargetEncodingRequiredV1
        || plan.physical_disposition
            != OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper frame or source custody drifted".into(),
        ));
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
    // The recipe belongs to the UEFI ProgramStorage slot, not to the x86-64
    // triple: every other admitted profile — including Windows x64, whose
    // `NativeTarget` is the same X86_64+Coff pair — selects a `HostedApplication`
    // slot whose authored arrival this recipe cannot replay. The slot identity
    // check is what keeps a same-triple contract from satisfying it.
    if contract.target() != target::NativeTarget::uefi_x64()
        || contract.target_slot() != target::TargetProfile::UefiX64.program_entry_slot()
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
        || placement.shape.byte_size != u32::from(EXTENT_BYTE_COUNT)
        || placement.shape.alignment != EXTENT_ALIGNMENT
        || !matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: Some(copy_offset),
                byte_size,
                alignment: EXTENT_ALIGNMENT,
            }] if *register == expected_register && *copy_offset == expected_copy_offset
                && *byte_size == u32::from(EXTENT_BYTE_COUNT)
        )
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "optimized semantic ProgramStorage {expected_role:?} placement drifted"
        )));
    }
    Ok(())
}

/// Replay the complete two-root transfer and its unchanged argument order.
fn replay_steps(
    plan: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let fingerprint = plan.source.semantic_calling_plan_report_fingerprint();
    if plan.steps != expected_steps(fingerprint) {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper action sequence drifted".into(),
        ));
    }
    Ok(())
}
