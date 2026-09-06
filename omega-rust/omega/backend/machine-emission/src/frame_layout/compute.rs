use selected_instructions::MachineAlternativeFamily;
use target::Architecture;

use crate::frame_layout::{
    AllocatedCalleeSavedFunctionKind, FrameAbiPreservationConvention,
    StagedOptimizedPostAllocationMachinePlan, ValidatedAllocatedCalleeSavedRequirements,
    ValidatedNonAuthoritativeCalleeSaveStorage, ValidatedTargetRegisterEnvironment,
};

use super::{
    CalleeSaveFrameSlot, FunctionTargetFrameLayout, ReturnAddressFrameCustody,
    TargetFrameLayoutError, TargetFrameLayoutPlan, TargetFrameLayoutPolicy,
};

pub(super) fn derive(
    machine: &StagedOptimizedPostAllocationMachinePlan,
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: TargetFrameLayoutPolicy,
) -> Result<TargetFrameLayoutPlan, TargetFrameLayoutError> {
    let machine_plan = machine.machine().plan();
    let requirement_plan = requirements.plan();
    let storage_plan = storage.plan();
    if machine_plan.target != environment.target()
        || machine_plan.register_environment != environment.identity()
        || machine_plan.physical_register_model != environment.physical().identity()
        || requirement_plan.selected != machine_plan.selected
        || requirement_plan.homes != machine_plan.homes
        || requirement_plan.post_allocation_manifest != machine_plan.post_allocation_manifest
        || requirement_plan.register_environment != environment.identity()
        || requirement_plan.physical_register_model != environment.physical().identity()
        || requirement_plan.target != environment.target()
        || storage_plan.callee_saved_requirements != requirements.receipt().identity()
        || storage_plan.register_environment != environment.identity()
        || storage_plan.physical_register_model != environment.physical().identity()
        || storage_plan.target != environment.target()
        || storage_plan.abi != requirement_plan.abi
    {
        return Err(TargetFrameLayoutError::RootMismatch);
    }
    if !machine_plan.structural_unit_functions.is_empty()
        || requirement_plan
            .functions
            .iter()
            .any(|row| row.kind != AllocatedCalleeSavedFunctionKind::Ordinary)
        || storage_plan
            .functions
            .iter()
            .any(|row| row.kind != AllocatedCalleeSavedFunctionKind::Ordinary)
    {
        return Err(TargetFrameLayoutError::StructuralFunctionUnsupported);
    }
    if machine_plan.functions.len() != requirement_plan.functions.len()
        || machine_plan.functions.len() != storage_plan.functions.len()
    {
        return Err(TargetFrameLayoutError::FunctionRosterMismatch);
    }

    let functions = machine_plan
        .functions
        .iter()
        .zip(&requirement_plan.functions)
        .zip(&storage_plan.functions)
        .map(
            |((machine_function, requirement_function), storage_function)| {
                if machine_function.machine != requirement_function.machine
                    || machine_function.machine != storage_function.machine
                    || requirement_function.kind != storage_function.kind
                {
                    return Err(TargetFrameLayoutError::FunctionRosterMismatch);
                }
                let contains_call = machine_function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|instruction| {
                        instruction.alternative.key.family == MachineAlternativeFamily::CallI64
                    });
                let callee_save_slots = storage_function
                    .slots
                    .iter()
                    .map(|slot| CalleeSaveFrameSlot {
                        abstract_slot: slot.id,
                        storage_view: slot.storage_view,
                        frame_offset_bytes: slot.abstract_offset_bytes,
                        size_bytes: slot.size_bytes,
                        alignment_bytes: slot.alignment_bytes,
                    })
                    .collect::<Vec<_>>();
                function_layout(
                    environment,
                    requirement_plan.abi,
                    policy,
                    machine_function.machine,
                    contains_call,
                    storage_function.abstract_area_bytes,
                    callee_save_slots,
                )
            },
        )
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TargetFrameLayoutPlan {
        post_allocation_machine: machine.machine().receipt().identity(),
        callee_saved_requirements: requirements.receipt().identity(),
        callee_save_storage: storage.receipt().identity(),
        register_environment: environment.identity(),
        physical_register_model: environment.physical().identity(),
        target: environment.target(),
        abi: requirement_plan.abi,
        policy,
        functions,
    })
}

fn function_layout(
    environment: &ValidatedTargetRegisterEnvironment,
    abi: FrameAbiPreservationConvention,
    policy: TargetFrameLayoutPolicy,
    machine: semantic_vocabulary::MachineId,
    contains_call: bool,
    callee_save_area_bytes: u64,
    callee_save_slots: Vec<CalleeSaveFrameSlot>,
) -> Result<FunctionTargetFrameLayout, TargetFrameLayoutError> {
    let (stack_pointer, frame_size_bytes, return_address) =
        match (environment.target().architecture, abi) {
            (
                Architecture::X86_64,
                convention @ (FrameAbiPreservationConvention::SystemVAMD64
                | FrameAbiPreservationConvention::MicrosoftX64),
            ) if !contains_call || convention == FrameAbiPreservationConvention::SystemVAMD64 => {
                let stack_pointer = environment
                    .physical()
                    .model()
                    .view_named("rsp")
                    .ok_or(TargetFrameLayoutError::MissingStackPointerView)?
                    .id;
                // A Windows leaf with no storage preserves the incoming RSP.
                // Once storage is allocated its body keeps the ABI alignment.
                // Calls need an outgoing home area and remain unsupported here.
                let frame_size = if contains_call
                    || (convention == FrameAbiPreservationConvention::MicrosoftX64
                        && callee_save_area_bytes != 0)
                {
                    align_to_residue(callee_save_area_bytes, 16, 8)?
                } else {
                    align_up(callee_save_area_bytes, 8)?
                };
                (
                    stack_pointer,
                    frame_size,
                    ReturnAddressFrameCustody::CallerActivationStack {
                        post_prologue_offset_bytes: frame_size,
                        size_bytes: 8,
                    },
                )
            }
            (
                Architecture::Aarch64,
                FrameAbiPreservationConvention::Aapcs64
                | FrameAbiPreservationConvention::DarwinAapcs64,
            ) => {
                let stack_pointer = environment
                    .physical()
                    .model()
                    .view_named("sp")
                    .ok_or(TargetFrameLayoutError::MissingStackPointerView)?
                    .id;
                let link = environment
                    .physical()
                    .model()
                    .view_named("x30")
                    .ok_or(TargetFrameLayoutError::MissingLinkRegisterView)?
                    .id;
                if contains_call
                    || policy == TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1
                {
                    let link_offset = align_up(callee_save_area_bytes, 8)?;
                    let used = link_offset
                        .checked_add(8)
                        .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
                    (
                        stack_pointer,
                        align_up(used, 16)?,
                        ReturnAddressFrameCustody::SavedLinkRegister {
                            view: link,
                            frame_offset_bytes: link_offset,
                            size_bytes: 8,
                        },
                    )
                } else {
                    (
                        stack_pointer,
                        align_up(callee_save_area_bytes, 16)?,
                        ReturnAddressFrameCustody::LiveLinkRegister { view: link },
                    )
                }
            }
            _ => return Err(TargetFrameLayoutError::UnsupportedTarget),
        };

    Ok(FunctionTargetFrameLayout {
        machine,
        kind: AllocatedCalleeSavedFunctionKind::Ordinary,
        contains_call,
        stack_pointer,
        pre_call_stack_alignment: 16,
        frame_size_bytes,
        abi_stack_alignment_bytes: 16,
        callee_save_slots,
        return_address,
    })
}

fn align_up(value: u64, alignment: u64) -> Result<u64, TargetFrameLayoutError> {
    let remainder = value % alignment;
    if remainder == 0 {
        return Ok(value);
    }
    value
        .checked_add(alignment - remainder)
        .ok_or(TargetFrameLayoutError::GeometryOverflow)
}

fn align_to_residue(
    value: u64,
    alignment: u64,
    residue: u64,
) -> Result<u64, TargetFrameLayoutError> {
    let current = value % alignment;
    let padding = (residue + alignment - current) % alignment;
    value
        .checked_add(padding)
        .ok_or(TargetFrameLayoutError::GeometryOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_leaf_storage_aligns_the_body_without_an_outgoing_home_area() {
        let environment = register_environment::baseline_target_register_environment(
            target::NativeTarget::windows_x64(),
        )
        .unwrap();
        for (area, extent) in [(0, 0), (8, 8), (16, 24)] {
            let layout = function_layout(
                &environment,
                FrameAbiPreservationConvention::MicrosoftX64,
                TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                semantic_vocabulary::MachineId::new(1).unwrap(),
                false,
                area,
                Vec::new(),
            )
            .unwrap();
            assert_eq!(layout.frame_size_bytes, extent);
            assert_eq!(
                layout.return_address,
                ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: extent,
                    size_bytes: 8,
                }
            );
        }
        assert_eq!(
            function_layout(
                &environment,
                FrameAbiPreservationConvention::MicrosoftX64,
                TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                semantic_vocabulary::MachineId::new(1).unwrap(),
                true,
                0,
                Vec::new(),
            ),
            Err(TargetFrameLayoutError::UnsupportedTarget)
        );
    }
}
