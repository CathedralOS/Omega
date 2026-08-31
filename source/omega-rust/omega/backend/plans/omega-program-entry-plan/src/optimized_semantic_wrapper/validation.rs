//! Independent replay of wrapper source custody, geometry, and action order.

use omega_calling_conventions::{
    CallingPolicy, IndirectPointerLocation, MachineRegister, ValueLocation, ValuePlacement,
};

use crate::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    ProgramEntrySourceExtentFieldRole, ProgramStorageEntryDiagnostic, ProgramStorageEntryRootRole,
};

use super::model::{
    OptimizedProgramStorageSemanticWrapperContinuationDisposition,
    OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    OptimizedProgramStorageSemanticWrapperPlan, OptimizedProgramStorageSemanticWrapperStep,
};
use super::recipe::{
    EXTENT_ALIGNMENT, EXTENT_BYTE_COUNT, OUTGOING_FRAME_BYTE_COUNT, PRE_CALL_STACK_ALIGNMENT,
    SHADOW_BYTE_COUNT, expected_relocation,
};

pub(super) fn validate(
    plan: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    validate_contract_surface(&plan.source)?;
    if plan.source_signature_identity != plan.source.source_signature_identity()
        || plan.shadow_byte_count != SHADOW_BYTE_COUNT
        || plan.outgoing_frame_byte_count != OUTGOING_FRAME_BYTE_COUNT
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
    if plan.relocation != expected_relocation() {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper call relocation drifted".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_contract_surface(
    contract: &OptimizedProgramStorageSemanticEntryContract,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if contract.target() != omega_target::NativeTarget::uefi_x64()
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

fn replay_steps(
    plan: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    use OptimizedProgramStorageSemanticWrapperStep as Step;
    let [
        Step::EnterFunction,
        Step::ReserveOutgoingStackFrame {
            byte_count: reserve,
        },
        image_base,
        image_length,
        storage_base,
        storage_length,
        image_address,
        storage_address,
        Step::CallPrivateTerminalContinuation {
            calling_policy,
            semantic_calling_plan_report_fingerprint,
            disposition,
        },
        Step::ReleaseOutgoingStackFrame {
            byte_count: release,
        },
        Step::ReturnUnit,
    ] = &plan.steps
    else {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper action sequence drifted".into(),
        ));
    };
    if *reserve != OUTGOING_FRAME_BYTE_COUNT
        || !replay_copy(
            image_base,
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntrySourceExtentFieldRole::Base,
            MachineRegister::X86Rcx,
            0,
            32,
        )
        || !replay_copy(
            image_length,
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntrySourceExtentFieldRole::Length,
            MachineRegister::X86Rcx,
            8,
            40,
        )
        || !replay_copy(
            storage_base,
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            ProgramEntrySourceExtentFieldRole::Base,
            MachineRegister::X86Rdx,
            0,
            48,
        )
        || !replay_copy(
            storage_length,
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            ProgramEntrySourceExtentFieldRole::Length,
            MachineRegister::X86Rdx,
            8,
            56,
        )
        || !replay_bind(
            image_address,
            ProgramStorageEntryRootRole::Image,
            0,
            MachineRegister::X86Rcx,
            32,
        )
        || !replay_bind(
            storage_address,
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            MachineRegister::X86Rdx,
            48,
        )
        || *calling_policy != CallingPolicy::MicrosoftX64
        || *semantic_calling_plan_report_fingerprint
            != plan.source.semantic_calling_plan_report_fingerprint()
        || *disposition
            != OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1
        || *release != OUTGOING_FRAME_BYTE_COUNT
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper action sequence drifted".into(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn replay_copy(
    step: &OptimizedProgramStorageSemanticWrapperStep,
    expected_role: ProgramStorageEntryRootRole,
    expected_parameter_index: usize,
    expected_field: ProgramEntrySourceExtentFieldRole,
    expected_register: MachineRegister,
    expected_source_offset: u16,
    expected_stack_offset: u32,
) -> bool {
    matches!(
        step,
        OptimizedProgramStorageSemanticWrapperStep::CopyIncomingIndirectExtentWord {
            role,
            parameter_index,
            field,
            source_register,
            source_byte_offset,
            outgoing_stack_byte_offset,
        } if *role == expected_role
            && *parameter_index == expected_parameter_index
            && *field == expected_field
            && *source_register == expected_register
            && *source_byte_offset == expected_source_offset
            && *outgoing_stack_byte_offset == expected_stack_offset
    )
}

fn replay_bind(
    step: &OptimizedProgramStorageSemanticWrapperStep,
    expected_role: ProgramStorageEntryRootRole,
    expected_parameter_index: usize,
    expected_register: MachineRegister,
    expected_stack_offset: u32,
) -> bool {
    matches!(
        step,
        OptimizedProgramStorageSemanticWrapperStep::BindOutgoingExtentCopyAddress {
            role,
            parameter_index,
            register,
            outgoing_stack_byte_offset,
            byte_count,
            alignment,
        } if *role == expected_role
            && *parameter_index == expected_parameter_index
            && *register == expected_register
            && *outgoing_stack_byte_offset == expected_stack_offset
            && *byte_count == EXTENT_BYTE_COUNT
            && *alignment == EXTENT_ALIGNMENT
    )
}
