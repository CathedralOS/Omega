use super::{
    CalleeSaveFrameSlot, FrameAbiPreservationConvention, ReturnAddressFrameCustody,
    TargetFrameLayoutError, TargetFrameLayoutPolicy, function_layout,
};
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
            Vec::new(),
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
            Vec::new(),
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
        let abi = register_environment::selected_abi_preservation(&environment)
            .unwrap()
            .kind;
        let layout = function_layout(
            &environment,
            abi,
            TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            true,
            &[],
            std::slice::from_ref(&local),
            Vec::new(),
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
                Vec::new(),
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
            Vec::new(),
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
                Vec::new(),
                0,
                Vec::new()
            )
            .is_err()
        );
    }
}

#[test]
fn large_frames_commit_through_an_exact_probe_roster() {
    let local = selected_instructions::SelectedLocalStorageSlot {
        id: selected_instructions::LocalStorageSlotId::Structural {
            operation: semantic_vocabulary::OperationId::new(7).unwrap(),
            place: semantic_vocabulary::PlaceId::new(11).unwrap(),
        },
        byte_size: 5_000,
        alignment: 8,
    };
    for (target, abi, interval, touches) in [
        (
            target::NativeTarget::linux_x64(),
            FrameAbiPreservationConvention::SystemVAMD64,
            4_096_u64,
            2_u32,
        ),
        (
            target::NativeTarget::windows_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
            4_096,
            2,
        ),
        (
            target::NativeTarget::uefi_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
            4_096,
            2,
        ),
        (
            target::NativeTarget::linux_arm64(),
            FrameAbiPreservationConvention::Aapcs64,
            4_096,
            2,
        ),
        (
            target::NativeTarget::macos_arm64(),
            FrameAbiPreservationConvention::DarwinAapcs64,
            16_384,
            0,
        ),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let layout = function_layout(
            &environment,
            abi,
            TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            false,
            &[],
            std::slice::from_ref(&local),
            Vec::new(),
            0,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(layout.stack_probe.interval_bytes, interval, "{target:?}");
        assert_eq!(layout.stack_probe.touches, touches, "{target:?}");
        // A frame inside one granule commits without probing.
        let small = selected_instructions::SelectedLocalStorageSlot {
            byte_size: 24,
            ..local.clone()
        };
        let small_layout = function_layout(
            &environment,
            abi,
            TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            false,
            &[],
            std::slice::from_ref(&small),
            Vec::new(),
            0,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(
            small_layout.stack_probe.interval_bytes, interval,
            "{target:?}"
        );
        assert_eq!(small_layout.stack_probe.touches, 0, "{target:?}");
    }
}

#[test]
fn system_v_leaf_spill_storage_inside_the_red_zone_commits_nothing() {
    let environment = register_environment::baseline_target_register_environment(
        target::NativeTarget::linux_x64(),
    )
    .unwrap();
    let spill = |register: u32| selected_instructions::SelectedLocalStorageSlot {
        id: selected_instructions::LocalStorageSlotId::Spill {
            register: selected_instructions::VirtualRegisterId(register),
        },
        byte_size: 8,
        alignment: 8,
    };
    for slots in [
        vec![spill(0)],
        vec![spill(0), spill(1), spill(2)],
        // Sixteen eight-byte slots sit exactly on the 128-byte red-zone
        // boundary: the entire addressed extent stays resident.
        (0..16).map(spill).collect::<Vec<_>>(),
    ] {
        let extent = u64::try_from(slots.len()).unwrap() * 8;
        let layout = function_layout(
            &environment,
            FrameAbiPreservationConvention::SystemVAMD64,
            TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            false,
            &[],
            &slots,
            Vec::new(),
            0,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(layout.frame_size_bytes, extent);
        assert_eq!(layout.red_zone_resident_bytes, extent);
        assert_eq!(
            layout.return_address,
            ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 0,
                size_bytes: 8,
            }
        );
        assert_eq!(layout.stack_probe.touches, 0);
    }
    // One slot past the boundary commits the whole frame again.
    let over: Vec<_> = (0..17).map(spill).collect();
    let layout = function_layout(
        &environment,
        FrameAbiPreservationConvention::SystemVAMD64,
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
        semantic_vocabulary::MachineId::new(1).unwrap(),
        false,
        &[],
        &over,
        Vec::new(),
        0,
        Vec::new(),
    )
    .unwrap();
    assert_eq!(layout.red_zone_resident_bytes, 0);
    assert_eq!(layout.frame_size_bytes, 136);
    assert_eq!(
        layout.return_address,
        ReturnAddressFrameCustody::CallerActivationStack {
            post_prologue_offset_bytes: 136,
            size_bytes: 8,
        }
    );
}

#[test]
fn red_zone_residency_requires_leaf_spill_only_sysv_storage() {
    let spill = |register: u32| selected_instructions::SelectedLocalStorageSlot {
        id: selected_instructions::LocalStorageSlotId::Spill {
            register: selected_instructions::VirtualRegisterId(register),
        },
        byte_size: 8,
        alignment: 8,
    };
    let structural = selected_instructions::SelectedLocalStorageSlot {
        id: selected_instructions::LocalStorageSlotId::Structural {
            operation: semantic_vocabulary::OperationId::new(7).unwrap(),
            place: semantic_vocabulary::PlaceId::new(11).unwrap(),
        },
        byte_size: 8,
        alignment: 8,
    };
    let sysv = register_environment::baseline_target_register_environment(
        target::NativeTarget::linux_x64(),
    )
    .unwrap();
    // A call, a hosted/structural slot, or any preservation storage each
    // independently force an ordinary committed frame.
    for (contains_call, locals, save_area, saves) in [
        (true, vec![spill(0)], 0, Vec::new()),
        (false, vec![structural.clone()], 0, Vec::new()),
        (false, vec![spill(0), structural.clone()], 0, Vec::new()),
        (
            false,
            vec![spill(0)],
            8,
            vec![CalleeSaveFrameSlot {
                abstract_slot: machine_code::NonAuthoritativeCalleeSaveSlotId(0),
                storage_view: sysv.physical().model().view_named("rbx").unwrap().id,
                frame_offset_bytes: 0,
                size_bytes: 8,
                alignment_bytes: 8,
            }],
        ),
    ] {
        let layout = function_layout(
            &sysv,
            FrameAbiPreservationConvention::SystemVAMD64,
            TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            contains_call,
            &[],
            &locals,
            Vec::new(),
            save_area,
            saves,
        )
        .unwrap();
        assert_eq!(layout.red_zone_resident_bytes, 0, "{locals:?}");
        assert!(layout.frame_size_bytes != 0);
        assert_eq!(
            layout.return_address,
            ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: layout.frame_size_bytes,
                size_bytes: 8,
            }
        );
    }
    // No other admitted ABI has a usable red zone.
    for (target, abi) in [
        (
            target::NativeTarget::windows_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
        ),
        (
            target::NativeTarget::uefi_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
        ),
        (
            target::NativeTarget::linux_arm64(),
            FrameAbiPreservationConvention::Aapcs64,
        ),
        (
            target::NativeTarget::macos_arm64(),
            FrameAbiPreservationConvention::DarwinAapcs64,
        ),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let layout = function_layout(
            &environment,
            abi,
            TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            false,
            &[],
            &[spill(0)],
            Vec::new(),
            0,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(layout.red_zone_resident_bytes, 0, "{target:?}");
        assert!(layout.frame_size_bytes != 0, "{target:?}");
    }
    // An empty leaf frame commits nothing and has nothing resident.
    let empty = function_layout(
        &sysv,
        FrameAbiPreservationConvention::SystemVAMD64,
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
        semantic_vocabulary::MachineId::new(1).unwrap(),
        false,
        &[],
        &[],
        Vec::new(),
        0,
        Vec::new(),
    )
    .unwrap();
    assert_eq!(empty.frame_size_bytes, 0);
    assert_eq!(empty.red_zone_resident_bytes, 0);
}

#[test]
fn unwind_matrix_fails_closed_on_conventions_the_pair_does_not_declare() {
    // Each declared (architecture, object-format) pair pins exactly one
    // preservation convention in its unwind row: a requirements plan
    // arriving under a convention another pair owns has drifted off the row
    // and fails closed instead of reaching a shared architecture arm.
    for (target, abi) in [
        (
            target::NativeTarget::linux_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
        ),
        (
            target::NativeTarget::windows_x64(),
            FrameAbiPreservationConvention::SystemVAMD64,
        ),
        (
            target::NativeTarget::uefi_x64(),
            FrameAbiPreservationConvention::SystemVAMD64,
        ),
        (
            target::NativeTarget::linux_arm64(),
            FrameAbiPreservationConvention::DarwinAapcs64,
        ),
        (
            target::NativeTarget::macos_arm64(),
            FrameAbiPreservationConvention::Aapcs64,
        ),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        assert_eq!(
            function_layout(
                &environment,
                abi,
                TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                semantic_vocabulary::MachineId::new(1).unwrap(),
                false,
                &[],
                &[],
                Vec::new(),
                0,
                Vec::new(),
            ),
            Err(TargetFrameLayoutError::UnsupportedTarget),
            "{target:?} must refuse the foreign convention {abi:?}"
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
            Vec::new(),
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
            Vec::new(),
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
            Vec::new(),
            u64::MAX,
            Vec::new(),
        ),
        Err(TargetFrameLayoutError::GeometryOverflow)
    );
}
