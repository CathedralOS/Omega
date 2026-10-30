use super::{
    CalleeSaveFrameSlot, FrameAbiPreservationConvention, ReturnAddressFrameCustody,
    TargetFrameLayoutError, TargetFrameLayoutPolicy, function_layout,
};

macro_rules! row {
    ($a:expr => $b:expr) => {
        ($a, $b)
    };
}

macro_rules! t {
    ($target:expr => $abi:ident => $c:expr => $d:expr) => {
        ($target, FrameAbiPreservationConvention::$abi, $c, $d)
    };
    ($target:expr => $abi:ident => $c:expr) => {
        ($target, FrameAbiPreservationConvention::$abi, $c)
    };
}

/// Every case here lays out a canonical ordinary-call frame for machine 1 with
/// no stable-address loans; the fixture supplies those three arguments so each
/// case names only the shape it is about.
fn frame_with_saves(
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    abi: FrameAbiPreservationConvention,
    contains_call: bool,
    writes_link_register: bool,
    outgoing_arguments: &[selected_instructions::SelectedOutgoingArgumentSlot],
    local_storage: &[selected_instructions::SelectedLocalStorageSlot],
    callee_save_area_bytes: u64,
    callee_save_slots: Vec<CalleeSaveFrameSlot>,
) -> Result<machine_code::FunctionTargetFrameLayout, TargetFrameLayoutError> {
    function_layout(
        environment,
        abi,
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
        semantic_vocabulary::MachineId::new(1).unwrap(),
        contains_call,
        writes_link_register,
        outgoing_arguments,
        local_storage,
        Vec::new(),
        callee_save_area_bytes,
        callee_save_slots,
    )
}

/// The same fixture for a case whose frame saves nothing: no callee-save area
/// and no callee-save slots.
fn frame(
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    abi: FrameAbiPreservationConvention,
    contains_call: bool,
    writes_link_register: bool,
    outgoing_arguments: &[selected_instructions::SelectedOutgoingArgumentSlot],
    local_storage: &[selected_instructions::SelectedLocalStorageSlot],
) -> Result<machine_code::FunctionTargetFrameLayout, TargetFrameLayoutError> {
    frame_with_saves(
        environment,
        abi,
        contains_call,
        writes_link_register,
        outgoing_arguments,
        local_storage,
        0,
        Vec::new(),
    )
}

#[test]
fn owned_overaligned_homes_keep_fixed_x64_stack_geometry() {
    use selected_instructions::{
        LocalStorageSlotId, OutgoingArgumentSlotId, OutgoingArgumentSlotRole,
        SelectedLocalStorageSlot, SelectedOutgoingArgumentSlot,
    };
    use semantic_vocabulary::{OperationId, PlaceId};
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let abi = register_environment::selected_abi_preservation(&environment)
            .unwrap()
            .kind;
        let outgoing = SelectedOutgoingArgumentSlot {
            id: OutgoingArgumentSlotId {
                role: OutgoingArgumentSlotRole::ValueCopy,
                invocation_site: OperationId::new(11).unwrap().into(),
                argument_index: 0,
            },
            byte_size: 32,
            alignment: 32,
            abi_stack_byte_offset: if target == target::NativeTarget::windows_x64() {
                32
            } else {
                0
            },
        };
        let local = SelectedLocalStorageSlot {
            id: LocalStorageSlotId::Structural {
                operation: OperationId::new(12).unwrap(),
                place: PlaceId::new(1).unwrap(),
            },
            byte_size: 64,
            alignment: 64,
        };
        let layout = frame(
            &environment,
            abi,
            true,
            false,
            std::slice::from_ref(&outgoing),
            std::slice::from_ref(&local),
        )
        .unwrap();
        assert_eq!(layout.abi_stack_alignment_bytes, 16);
        assert_eq!(layout.frame_size_bytes % 16, 8);
        assert_eq!(layout.red_zone_resident_bytes, 0);
        let placed = &layout.local_storage_slots[0];
        let outgoing_end = u64::from(outgoing.abi_stack_byte_offset)
            + u64::from(outgoing.reserved_byte_size(16).unwrap());
        assert!(placed.frame_offset_bytes >= outgoing_end);
        assert!(
            placed.frame_offset_bytes + placed.reserved_size_bytes(16) <= layout.frame_size_bytes
        );
        for (start, size, alignment, reserved) in [
            (
                u64::from(outgoing.abi_stack_byte_offset),
                32_u64,
                32_u64,
                u64::from(outgoing.reserved_byte_size(16).unwrap()),
            ),
            (
                placed.frame_offset_bytes,
                64,
                64,
                placed.reserved_size_bytes(16),
            ),
        ] {
            for stack_residue in (0..alignment).step_by(16) {
                let base = stack_residue + start;
                let address = (base + alignment - 1) & !(alignment - 1);
                assert_eq!(address % alignment, 0);
                assert!(address >= base && address + size <= base + reserved);
            }
        }
        // ABI argument coordinates cannot be rounded like private copies.
        let mut invalid = outgoing;
        invalid.id.role = OutgoingArgumentSlotRole::Argument;
        assert_eq!(
            frame(&environment, abi, true, false, &[invalid], &[],),
            Err(TargetFrameLayoutError::NonCanonicalLayout),
        );
    }
}

#[test]
fn narrow_outgoing_payload_aligns_preservation_without_activation_locals() {
    for (target, abi, stack_offset, save_name) in [
        t!(target::NativeTarget::linux_x64() => SystemVAMD64 => 0 => "rbx"),
        t!(target::NativeTarget::windows_x64() => MicrosoftX64 => 32 => "rbx"),
        t!(target::NativeTarget::linux_arm64() => Aapcs64 => 0 => "x19"),
        t!(target::NativeTarget::macos_arm64() => DarwinAapcs64 => 0 => "x19"),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let outgoing = selected_instructions::SelectedOutgoingArgumentSlot {
            id: selected_instructions::OutgoingArgumentSlotId {
                role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                invocation_site: semantic_vocabulary::OperationId::new(11).unwrap().into(),
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
        let layout = frame_with_saves(
            &environment,
            abi,
            true,
            false,
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
        let without_save = frame(&environment, abi, true, false, &[outgoing], &[]).unwrap();
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
        let layout = frame(
            &environment,
            abi,
            true,
            false,
            &[],
            std::slice::from_ref(&local),
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
            frame(
                &environment,
                abi,
                true,
                false,
                &[],
                &[local.clone(), local.clone()],
            )
            .is_err()
        );
    }
}

#[test]
fn outgoing_pointer_slots_reserve_storage_on_every_host_abi() {
    for (target, abi, offset) in [
        t!(target::NativeTarget::windows_x64() => MicrosoftX64 => 32),
        t!(target::NativeTarget::linux_x64() => SystemVAMD64 => 0),
        t!(target::NativeTarget::linux_arm64() => Aapcs64 => 0),
        t!(target::NativeTarget::macos_arm64() => DarwinAapcs64 => 0),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let slot = selected_instructions::SelectedOutgoingArgumentSlot {
            id: selected_instructions::OutgoingArgumentSlotId {
                role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                invocation_site: semantic_vocabulary::OperationId::new(11).unwrap().into(),
                argument_index: 8,
            },
            byte_size: 8,
            alignment: 8,
            abi_stack_byte_offset: offset,
        };
        let layout = frame(
            &environment,
            abi,
            true,
            false,
            std::slice::from_ref(&slot),
            &[],
        )
        .unwrap();
        assert_eq!(layout.outgoing_abi_area.byte_size, u64::from(offset) + 8);
        assert!(layout.frame_size_bytes >= u64::from(offset) + 8);
        assert!(frame(&environment, abi, false, false, &[slot], &[]).is_err());
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
        t!(target::NativeTarget::linux_x64() => SystemVAMD64 => 4_096_u64 => 2_u32),
        t!(target::NativeTarget::windows_x64() => MicrosoftX64 => 4_096 => 2),
        t!(target::NativeTarget::uefi_x64() => MicrosoftX64 => 4_096 => 2),
        t!(target::NativeTarget::linux_arm64() => Aapcs64 => 4_096 => 2),
        t!(target::NativeTarget::macos_arm64() => DarwinAapcs64 => 16_384 => 0),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let layout = frame(
            &environment,
            abi,
            false,
            false,
            &[],
            std::slice::from_ref(&local),
        )
        .unwrap();
        assert_eq!(layout.stack_probe.interval_bytes, interval, "{target:?}");
        assert_eq!(layout.stack_probe.touches, touches, "{target:?}");
        // A frame inside one granule commits without probing.
        let small = selected_instructions::SelectedLocalStorageSlot {
            byte_size: 24,
            ..local.clone()
        };
        let small_layout = frame(
            &environment,
            abi,
            false,
            false,
            &[],
            std::slice::from_ref(&small),
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
fn frames_past_the_shared_displacement_reach_are_refused_by_name() {
    for (target, abi) in [
        row!(target::NativeTarget::linux_x64() => FrameAbiPreservationConvention::SystemVAMD64),
        row!(target::NativeTarget::windows_x64() => FrameAbiPreservationConvention::MicrosoftX64),
        row!(target::NativeTarget::linux_arm64() => FrameAbiPreservationConvention::Aapcs64),
        row!(target::NativeTarget::macos_arm64() => FrameAbiPreservationConvention::DarwinAapcs64),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let layout = |byte_size: u32| {
            frame(
                &environment,
                abi,
                false,
                false,
                &[],
                &[selected_instructions::SelectedLocalStorageSlot {
                    id: selected_instructions::LocalStorageSlotId::Structural {
                        operation: semantic_vocabulary::OperationId::new(7).unwrap(),
                        place: semantic_vocabulary::PlaceId::new(11).unwrap(),
                    },
                    byte_size,
                    alignment: 8,
                }],
            )
        };
        // 32 MiB and the last page under 2 GiB lay out; 2 GiB does not.
        assert!(layout(33_554_440).unwrap().frame_size_bytes >= 33_554_440);
        assert!(layout(0x7fff_f000).unwrap().frame_size_bytes <= super::super::MAX_FRAME_BYTES);
        assert!(matches!(
            layout(0x8000_0000),
            Err(TargetFrameLayoutError::FrameExceedsAddressableExtent {
                frame_size_bytes
            }) if frame_size_bytes > super::super::MAX_FRAME_BYTES
        ));
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
        let layout = frame(
            &environment,
            FrameAbiPreservationConvention::SystemVAMD64,
            false,
            false,
            &[],
            &slots,
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
    let layout = frame(
        &environment,
        FrameAbiPreservationConvention::SystemVAMD64,
        false,
        false,
        &[],
        &over,
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
        let layout = frame_with_saves(
            &sysv,
            FrameAbiPreservationConvention::SystemVAMD64,
            contains_call,
            false,
            &[],
            &locals,
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
        row!(target::NativeTarget::windows_x64() => FrameAbiPreservationConvention::MicrosoftX64),
        row!(target::NativeTarget::uefi_x64() => FrameAbiPreservationConvention::MicrosoftX64),
        row!(target::NativeTarget::linux_arm64() => FrameAbiPreservationConvention::Aapcs64),
        row!(target::NativeTarget::macos_arm64() => FrameAbiPreservationConvention::DarwinAapcs64),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let layout = frame(&environment, abi, false, false, &[], &[spill(0)]).unwrap();
        assert_eq!(layout.red_zone_resident_bytes, 0, "{target:?}");
        assert!(layout.frame_size_bytes != 0, "{target:?}");
    }
    // An empty leaf frame commits nothing and has nothing resident.
    let empty = frame(
        &sysv,
        FrameAbiPreservationConvention::SystemVAMD64,
        false,
        false,
        &[],
        &[],
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
        row!(target::NativeTarget::linux_x64() => FrameAbiPreservationConvention::MicrosoftX64),
        row!(target::NativeTarget::windows_x64() => FrameAbiPreservationConvention::SystemVAMD64),
        row!(target::NativeTarget::uefi_x64() => FrameAbiPreservationConvention::SystemVAMD64),
        row!(target::NativeTarget::linux_arm64() => FrameAbiPreservationConvention::DarwinAapcs64),
        row!(target::NativeTarget::macos_arm64() => FrameAbiPreservationConvention::Aapcs64),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        assert_eq!(
            frame(&environment, abi, false, false, &[], &[]),
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
        let layout = frame_with_saves(
            &environment,
            FrameAbiPreservationConvention::MicrosoftX64,
            false,
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
        let layout = frame_with_saves(
            &environment,
            FrameAbiPreservationConvention::MicrosoftX64,
            true,
            false,
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
        frame_with_saves(
            &environment,
            FrameAbiPreservationConvention::MicrosoftX64,
            true,
            false,
            &[],
            &[],
            u64::MAX,
            Vec::new()
        ),
        Err(TargetFrameLayoutError::GeometryOverflow)
    );
}

#[test]
fn allocated_link_register_write_gets_a_leaf_frame_slot() {
    for target in [
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let link = environment.physical().model().view_named("x30").unwrap().id;
        let abi = register_environment::selected_abi_preservation(&environment)
            .unwrap()
            .kind;
        let layout = frame(&environment, abi, false, true, &[], &[]).unwrap();
        assert!(!layout.contains_call);
        assert_eq!(layout.frame_size_bytes, 16);
        assert_eq!(
            layout.return_address,
            ReturnAddressFrameCustody::SavedLinkRegister {
                view: link,
                frame_offset_bytes: 0,
                size_bytes: 8,
            }
        );
    }
}
