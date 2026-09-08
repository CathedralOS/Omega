use selected_instructions::MachineAlternativeFamily;
use target::Architecture;

use crate::frame_layout::{
    FrameAbiPreservationConvention, StagedOptimizedPostAllocationMachinePlan,
    ValidatedAllocatedCalleeSavedRequirements, ValidatedNonAuthoritativeCalleeSaveStorage,
    ValidatedTargetRegisterEnvironment,
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
                {
                    return Err(TargetFrameLayoutError::FunctionRosterMismatch);
                }
                let contains_call = machine_function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|instruction| {
                        matches!(
                            instruction.alternative.key.family,
                            MachineAlternativeFamily::CallI64 | MachineAlternativeFamily::CallUnit
                        )
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
                    &machine_function.outgoing_arguments,
                    &machine_function.local_storage_slots,
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
    outgoing_arguments: &[selected_instructions::SelectedOutgoingArgumentSlot],
    local_storage: &[selected_instructions::SelectedLocalStorageSlot],
    callee_save_area_bytes: u64,
    mut callee_save_slots: Vec<CalleeSaveFrameSlot>,
) -> Result<FunctionTargetFrameLayout, TargetFrameLayoutError> {
    let shadow_bytes = if contains_call && abi == FrameAbiPreservationConvention::MicrosoftX64 {
        32
    } else {
        0
    };
    let mut outgoing_extent = u64::from(shadow_bytes);
    for slot in outgoing_arguments {
        if !contains_call
            || abi != FrameAbiPreservationConvention::MicrosoftX64
            || slot.byte_size == 0
            || !slot.alignment.is_power_of_two()
            || slot.abi_stack_byte_offset < u32::from(shadow_bytes)
            || !slot
                .abi_stack_byte_offset
                .is_multiple_of(u32::from(slot.alignment))
        {
            return Err(TargetFrameLayoutError::NonCanonicalLayout);
        }
        let end = slot
            .abi_stack_byte_offset
            .checked_add(slot.byte_size)
            .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
        outgoing_extent = outgoing_extent.max(u64::from(end));
    }
    let outgoing_abi_area = machine_code::OutgoingAbiFrameArea {
        byte_size: outgoing_extent,
        shadow_bytes,
    };
    let mut local_extent = outgoing_extent;
    let mut local_storage_slots = Vec::with_capacity(local_storage.len());
    for (index, slot) in local_storage.iter().enumerate() {
        if slot.byte_size == 0
            || !slot.alignment.is_power_of_two()
            || slot.alignment > 8
            || local_storage[..index]
                .iter()
                .any(|earlier| earlier.id == slot.id)
        {
            return Err(TargetFrameLayoutError::NonCanonicalLayout);
        }
        let frame_offset_bytes = align_up(local_extent, u64::from(slot.alignment))?;
        local_extent = frame_offset_bytes
            .checked_add(u64::from(slot.byte_size))
            .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
        local_storage_slots.push(machine_code::LocalStorageFrameSlot {
            id: slot.id,
            frame_offset_bytes,
            size_bytes: slot.byte_size,
            alignment_bytes: slot.alignment,
        });
    }
    // Preservation storage retains its own alignment after arbitrary local extents.
    let preservation_offset = if local_storage_slots.is_empty() {
        outgoing_extent
    } else {
        let preservation_alignment = callee_save_slots
            .iter()
            .map(|slot| slot.alignment_bytes)
            .max()
            .unwrap_or(8)
            .max(8);
        if !preservation_alignment.is_power_of_two() {
            return Err(TargetFrameLayoutError::NonCanonicalLayout);
        }
        align_up(local_extent, preservation_alignment)?
    };
    for slot in &mut callee_save_slots {
        slot.frame_offset_bytes = slot
            .frame_offset_bytes
            .checked_add(preservation_offset)
            .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
    }
    let used_area_bytes = callee_save_area_bytes
        .checked_add(preservation_offset)
        .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
    let (stack_pointer, frame_size_bytes, return_address) =
        match (environment.target().architecture, abi) {
            (
                Architecture::X86_64,
                convention @ (FrameAbiPreservationConvention::SystemVAMD64
                | FrameAbiPreservationConvention::MicrosoftX64),
            ) => {
                let stack_pointer = environment
                    .physical()
                    .model()
                    .view_named("rsp")
                    .ok_or(TargetFrameLayoutError::MissingStackPointerView)?
                    .id;
                // A Windows leaf with no storage preserves the incoming RSP.
                // Once storage is allocated its body keeps the ABI alignment.
                // Outgoing ABI storage precedes all preservation storage.
                let frame_size = if contains_call
                    || (convention == FrameAbiPreservationConvention::MicrosoftX64
                        && used_area_bytes != 0)
                {
                    align_to_residue(used_area_bytes, 16, 8)?
                } else {
                    align_up(used_area_bytes, 8)?
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
                    let link_offset = align_up(used_area_bytes, 8)?;
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
                        align_up(used_area_bytes, 16)?,
                        ReturnAddressFrameCustody::LiveLinkRegister { view: link },
                    )
                }
            }
            _ => return Err(TargetFrameLayoutError::UnsupportedTarget),
        };

    Ok(FunctionTargetFrameLayout {
        machine,
        contains_call,
        stack_pointer,
        pre_call_stack_alignment: 16,
        frame_size_bytes,
        abi_stack_alignment_bytes: 16,
        outgoing_abi_area,
        local_storage_slots,
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
    fn activation_locals_are_not_outgoing_abi_storage_on_any_host() {
        let id = selected_instructions::LocalStorageSlotId::Structural {
            operation: semantic_vocabulary::OperationId::new(7).unwrap(),
            place: semantic_vocabulary::PlaceId::new(11).unwrap(),
        };
        let local = selected_instructions::SelectedLocalStorageSlot {
            id,
            byte_size: 24,
            alignment: 8,
        };
        for target in [
            target::NativeTarget::windows_x64(),
            target::NativeTarget::linux_x64(),
            target::NativeTarget::linux_arm64(),
            target::NativeTarget::macos_arm64(),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let abi = match (target.architecture, target.object_format) {
                (target::Architecture::X86_64, target::ObjectFormat::Coff) => {
                    FrameAbiPreservationConvention::MicrosoftX64
                }
                (target::Architecture::X86_64, _) => FrameAbiPreservationConvention::SystemVAMD64,
                (_, target::ObjectFormat::MachO) => FrameAbiPreservationConvention::DarwinAapcs64,
                _ => FrameAbiPreservationConvention::Aapcs64,
            };
            let layout = function_layout(
                &environment,
                abi,
                TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                semantic_vocabulary::MachineId::new(1).unwrap(),
                true,
                &[],
                std::slice::from_ref(&local),
                0,
                Vec::new(),
            )
            .unwrap();
            let outgoing = if abi == FrameAbiPreservationConvention::MicrosoftX64 {
                32
            } else {
                0
            };
            assert_eq!(layout.outgoing_abi_area.byte_size, outgoing);
            assert_eq!(layout.local_storage_slots[0].id, id);
            assert_eq!(layout.local_storage_slots[0].frame_offset_bytes, outgoing);
            assert_eq!(layout.local_storage_slots[0].size_bytes, 24);
            assert!(layout.frame_size_bytes >= outgoing + 24);
            assert!(
                function_layout(
                    &environment,
                    abi,
                    TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                    semantic_vocabulary::MachineId::new(1).unwrap(),
                    true,
                    &[],
                    &[local.clone(), local.clone()],
                    0,
                    Vec::new()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn windows_frames_separate_shadow_space_from_preservation_storage() {
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
                &[],
                &[],
                area,
                Vec::new(),
            )
            .unwrap();
            assert_eq!(layout.frame_size_bytes, extent);
            assert_eq!(layout.outgoing_abi_area.byte_size, 0);
            assert_eq!(layout.outgoing_abi_area.shadow_bytes, 0);
            assert_eq!(
                layout.return_address,
                ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: extent,
                    size_bytes: 8,
                }
            );
        }
        for (area, expected_extent) in [(0, 40), (8, 40), (16, 56)] {
            let slots = if area == 0 {
                Vec::new()
            } else {
                vec![CalleeSaveFrameSlot {
                    abstract_slot: machine_code::NonAuthoritativeCalleeSaveSlotId(0),
                    storage_view: environment.physical().model().view_named("rbx").unwrap().id,
                    frame_offset_bytes: 0,
                    size_bytes: area,
                    alignment_bytes: 8,
                }]
            };
            let layout = function_layout(
                &environment,
                FrameAbiPreservationConvention::MicrosoftX64,
                TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                semantic_vocabulary::MachineId::new(1).unwrap(),
                true,
                &[],
                &[],
                area,
                slots,
            )
            .unwrap();
            assert_eq!(layout.outgoing_abi_area.byte_size, 32);
            assert_eq!(layout.outgoing_abi_area.shadow_bytes, 32);
            assert_eq!(layout.frame_size_bytes, expected_extent);
            assert!(
                layout
                    .callee_save_slots
                    .iter()
                    .all(|slot| slot.frame_offset_bytes == 32)
            );
        }
        assert_eq!(
            function_layout(
                &environment,
                FrameAbiPreservationConvention::MicrosoftX64,
                TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                semantic_vocabulary::MachineId::new(1).unwrap(),
                true,
                &[],
                &[],
                u64::MAX,
                Vec::new(),
            ),
            Err(TargetFrameLayoutError::GeometryOverflow)
        );
    }
}
