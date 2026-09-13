use super::*;

#[test]
fn narrow_outgoing_payload_aligns_preservation_without_activation_locals() {
    for (target, abi, stack_offset, save_name) in [
        (
            target::NativeTarget::linux_x64(),
            FrameAbiPreservationConvention::SystemVAMD64,
            0,
            "rbx",
        ),
        (
            target::NativeTarget::windows_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
            32,
            "rbx",
        ),
        (
            target::NativeTarget::linux_arm64(),
            FrameAbiPreservationConvention::Aapcs64,
            0,
            "x19",
        ),
        (
            target::NativeTarget::macos_arm64(),
            FrameAbiPreservationConvention::DarwinAapcs64,
            0,
            "x19",
        ),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let outgoing = selected_instructions::SelectedOutgoingArgumentSlot {
            id: selected_instructions::OutgoingArgumentSlotId {
                role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                operation: semantic_vocabulary::OperationId::new(11).unwrap(),
                argument_index: 8,
            },
            byte_size: 4,
            alignment: 4,
            abi_stack_byte_offset: stack_offset,
        };
        let save = CalleeSaveFrameSlot {
            abstract_slot: machine_code::NonAuthoritativeCalleeSaveSlotId(0),
            storage_view: environment
                .physical()
                .model()
                .view_named(save_name)
                .unwrap()
                .id,
            frame_offset_bytes: 0,
            size_bytes: 8,
            alignment_bytes: 8,
        };
        let layout = function_layout(
            &environment,
            abi,
            TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            true,
            std::slice::from_ref(&outgoing),
            &[],
            8,
            vec![save],
        )
        .unwrap();
        let outgoing_end = u64::from(stack_offset) + 4;
        assert_eq!(layout.outgoing_abi_area.byte_size, outgoing_end);
        assert!(layout.local_storage_slots.is_empty());
        let saved = &layout.callee_save_slots[0];
        assert_eq!(saved.frame_offset_bytes, outgoing_end + 4);
        assert!(
            saved
                .frame_offset_bytes
                .is_multiple_of(saved.alignment_bytes)
        );
        assert!(saved.frame_offset_bytes + saved.size_bytes <= layout.frame_size_bytes);

        // Without preservation storage the payload does not acquire invented padding.
        let without_save = function_layout(
            &environment,
            abi,
            TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            true,
            &[outgoing],
            &[],
            0,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(without_save.outgoing_abi_area.byte_size, outgoing_end);
        assert!(without_save.callee_save_slots.is_empty());
    }
}

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
fn outgoing_pointer_slots_reserve_storage_on_every_host_abi() {
    for (target, abi, offset) in [
        (
            target::NativeTarget::windows_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
            32,
        ),
        (
            target::NativeTarget::linux_x64(),
            FrameAbiPreservationConvention::SystemVAMD64,
            0,
        ),
        (
            target::NativeTarget::linux_arm64(),
            FrameAbiPreservationConvention::Aapcs64,
            0,
        ),
        (
            target::NativeTarget::macos_arm64(),
            FrameAbiPreservationConvention::DarwinAapcs64,
            0,
        ),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let slot = selected_instructions::SelectedOutgoingArgumentSlot {
            id: selected_instructions::OutgoingArgumentSlotId {
                role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                operation: semantic_vocabulary::OperationId::new(11).unwrap(),
                argument_index: 8,
            },
            byte_size: 8,
            alignment: 8,
            abi_stack_byte_offset: offset,
        };
        let layout = function_layout(
            &environment,
            abi,
            TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            true,
            std::slice::from_ref(&slot),
            &[],
            0,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(layout.outgoing_abi_area.byte_size, u64::from(offset) + 8);
        assert!(layout.frame_size_bytes >= u64::from(offset) + 8);
        assert!(
            function_layout(
                &environment,
                abi,
                TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                semantic_vocabulary::MachineId::new(1).unwrap(),
                false,
                &[slot],
                &[],
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
