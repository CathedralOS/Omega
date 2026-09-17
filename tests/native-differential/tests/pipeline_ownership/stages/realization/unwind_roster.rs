//! A caller keeping a parameter and earlier call results live across later
//! calls commits preservation storage and, on AAPCS64, a saved link register.
//! The validated frame layout records the exact unwind roster — the ordered
//! view restorations the epilogue performs (the saved link register first,
//! then preservation slots in reverse save order), the committed extent the
//! unwind releases, and the return-address custody the continuation is
//! recovered through. Replay recovers the same roster from the retained
//! storage plan and custody rather than the producer's claims; an omitted,
//! invented, reordered, or misattributed restore — or a wrong release or
//! custody restatement — fails closed, while the emitted epilogue performs
//! exactly the recorded roster and the artifact still reaches ordinary
//! callable publication on every admitted target.

use crate::tests::{
    AdmissionProfile, FunctionTargetFrameLayout, MachineId, NativeTarget, OptimizationSelections,
    ReturnAddressFrameCustody, canonical_artifact, compiler_baseline_request_v1,
    optimize_artifact_sections, scalar_call_preserving_artifact,
    stage_function_fragment_frame_application, stage_optimized_fixed_frame_text_section,
    stage_optimized_function_fragment_emission, stage_optimized_relocation_free_object_container,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
    stage_validated_optimized_object_artifact, stage_validated_optimized_ordinary_callable_entry,
    validate_optimized_ordinary_callable_entry, validate_target_frame_layout,
    validate_target_frame_protocol_encoding,
};

/// Decode the restorations an x86-64 epilogue performs before its stack
/// release: one `mov reg, [rsp + offset]` per roster row in emitted order,
/// then the `add rsp` releasing the committed extent. Returns the
/// `(register name, frame offset)` roster and the released byte count.
fn x86_64_epilogue(epilogue: &[u8]) -> (Vec<(String, u64)>, u64) {
    let mut restores = Vec::new();
    let mut cursor = 0;
    while cursor + 4 <= epilogue.len() && epilogue[cursor + 1] == 0x8b {
        let rex = epilogue[cursor];
        assert!(
            matches!(rex, 0x48 | 0x4c),
            "rex {rex:#x} in {epilogue:02x?}"
        );
        let modrm = epilogue[cursor + 2];
        assert_eq!(modrm & 0b111, 0b100, "SIB follows");
        assert_eq!(epilogue[cursor + 3], 0x24, "rsp-based SIB");
        let code = (modrm >> 3) & 7 | ((rex & 4) << 1);
        let (offset, width) = match modrm >> 6 {
            0b00 => (0, 4),
            0b01 => (u64::from(epilogue[cursor + 4]), 5),
            0b10 => (
                u64::from(u32::from_le_bytes(
                    epilogue[cursor + 4..cursor + 8].try_into().unwrap(),
                )),
                8,
            ),
            _ => unreachable!(),
        };
        restores.push((x86_64_register_name(code), offset));
        cursor += width;
    }
    let released = match &epilogue[cursor..] {
        [] => 0,
        [0x48, 0x83, 0xc4, immediate] => u64::from(*immediate),
        [0x48, 0x81, 0xc4, rest @ ..] => u64::from(u32::from_le_bytes(rest.try_into().unwrap())),
        tail => panic!("epilogue must end with the stack release: {tail:02x?}"),
    };
    (restores, released)
}

fn x86_64_register_name(code: u8) -> String {
    match code {
        0..=7 => {
            ["rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi"][usize::from(code)].to_string()
        }
        _ => format!("r{code}"),
    }
}

/// Decode the restorations an AAPCS64 epilogue performs before its stack
/// release: one `ldr` per roster row in emitted order, then the shifted or
/// unshifted `add sp` pair releasing the committed extent. Returns the
/// `(register name, frame offset)` roster and the released byte count.
fn aarch64_epilogue(epilogue: &[u8]) -> (Vec<(String, u64)>, u64) {
    let mut restore_words = epilogue
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes(*chunk))
        .collect::<Vec<_>>();
    let mut released = 0_u64;
    while let Some(&last) = restore_words.last() {
        if last & 0xff00_03ff != 0x9100_03ff {
            break;
        }
        released += u64::from((last >> 10) & 0xfff) << (((last >> 22) & 1) * 12);
        restore_words.pop();
    }
    let restores = restore_words
        .iter()
        .map(|word| {
            let load = word & 0xffc0_03e0;
            let prefix = match load {
                0xf940_03e0 => "x",
                0xfd40_03e0 => "d",
                _ => panic!("expected a slot restore: {word:#010x}"),
            };
            (
                format!("{prefix}{}", word & 0x1f),
                u64::from((word >> 10) & 0xfff) * 8,
            )
        })
        .collect();
    (restores, released)
}

/// Recover the `(register name, frame offset)` roster a canonical layout
/// must record for one function, from its preservation storage and
/// return-address custody rather than its recorded unwind field: a saved
/// link register heads the list, then every preservation slot in reverse
/// save order, so the roster is strictly descending frame offsets.
fn expected_roster(
    row: &FunctionTargetFrameLayout,
    view_name: &dyn Fn(register_model::RegisterViewId) -> String,
) -> Vec<(String, u64)> {
    let mut expected = Vec::with_capacity(row.callee_save_slots.len() + 1);
    if let ReturnAddressFrameCustody::SavedLinkRegister {
        view,
        frame_offset_bytes,
        ..
    } = row.return_address
    {
        expected.push((view_name(view), frame_offset_bytes));
    }
    expected.extend(
        row.callee_save_slots
            .iter()
            .rev()
            .map(|slot| (view_name(slot.storage_view), slot.frame_offset_bytes)),
    );
    expected
}

#[test]
fn unwind_restore_roster_replays_through_ordinary_callable_entry() {
    let caller = MachineId::new(crate::tests::SCALAR_CALL_PRESERVING_CALLER).unwrap();
    let (semantic, proof) = scalar_call_preserving_artifact();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&OptimizationSelections::new([]).unwrap()),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap_or_else(|error| panic!("{target:?} physical: {error:?}"));
        let realization = physical.fixed_frame_for_test();
        let layout = realization.frame().plan();
        let current = realization.allocation().current();
        let physical_model = current.register_environment().physical();
        let view_name = |view: register_model::RegisterViewId| {
            physical_model
                .model()
                .views
                .iter()
                .find(|entry| entry.id == view)
                .unwrap_or_else(|| panic!("unknown view {view:?}"))
                .name
                .clone()
        };
        let row = layout
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap_or_else(|| panic!("{target:?}: missing caller frame row"));
        // The caller's non-leaf frame carries preservation storage on every
        // target; on AAPCS64 it also saves the incoming link register.
        assert!(row.contains_call, "{target:?}");
        assert!(
            !row.callee_save_slots.is_empty(),
            "{target:?}: live-across-call values must occupy preservation storage"
        );
        match (target.architecture, row.return_address) {
            (
                target::Architecture::X86_64,
                ReturnAddressFrameCustody::CallerActivationStack { .. },
            ) => {}
            (
                target::Architecture::Aarch64,
                ReturnAddressFrameCustody::SavedLinkRegister { .. },
            ) => {}
            custody => panic!("{target:?}: unexpected return-address custody {custody:?}"),
        }
        // The recorded roster is the epilogue's restoration sequence — saved
        // link first, then preservation slots in strictly descending frame
        // offsets — and it releases exactly the committed extent while
        // restating the checked custody.
        let expected = expected_roster(row, &view_name);
        let recorded = row
            .unwind
            .restores
            .iter()
            .map(|restore| (view_name(restore.view), restore.frame_offset_bytes))
            .collect::<Vec<_>>();
        assert_eq!(recorded, expected, "{target:?} {row:?}");
        assert!(
            row.unwind
                .restores
                .windows(2)
                .all(|pair| pair[0].frame_offset_bytes > pair[1].frame_offset_bytes),
            "{target:?}: the roster is not in descending frame offsets: {row:?}"
        );
        for restore in &row.unwind.restores {
            assert!(
                restore.frame_offset_bytes + restore.size_bytes <= row.frame_size_bytes,
                "{target:?}: restore {restore:?} escapes the frame"
            );
        }
        assert_eq!(
            row.unwind.released_bytes,
            row.frame_size_bytes - row.red_zone_resident_bytes,
            "{target:?}: the release must be the committed extent"
        );
        assert_eq!(
            row.unwind.return_address, row.return_address,
            "{target:?}: the roster must restate the frame custody"
        );
        // The leaf callee needs no restorations: an empty roster releasing
        // its (possibly empty) frame is a complete unwind description too.
        let callee_row = layout
            .functions
            .iter()
            .find(|function| function.machine != caller)
            .unwrap_or_else(|| panic!("{target:?}: missing callee frame row"));
        assert!(callee_row.unwind.restores.is_empty(), "{target:?}");
        assert_eq!(
            callee_row.unwind.return_address, callee_row.return_address,
            "{target:?}"
        );
        // The emitted epilogue performs exactly the recorded roster and
        // releases exactly the committed extent — the protocol consumed the
        // validated unwind, not an independently reconstructed save list.
        let protocol = realization.protocol().plan();
        let encoding = protocol
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap();
        let epilogue = encoding.epilogue.bytes(&protocol.bytes).unwrap();
        let (performed, released) = match target.architecture {
            target::Architecture::X86_64 => x86_64_epilogue(epilogue),
            target::Architecture::Aarch64 => aarch64_epilogue(epilogue),
        };
        assert_eq!(
            performed, expected,
            "{target:?}: epilogue performs a different roster: {epilogue:02x?}"
        );
        assert_eq!(
            released, row.unwind.released_bytes,
            "{target:?}: epilogue releases a different extent: {epilogue:02x?}"
        );
        // The produced layout replays exactly; a roster that omits a
        // restore, invents one, reorders the sequence, misattributes the
        // slot, understates the release, or restates different custody
        // fails closed on every target.
        let plan = layout.clone();
        let validate = |candidate: machine_code::TargetFrameLayoutPlan| {
            validate_target_frame_layout(
                realization.machine(),
                realization.requirements(),
                realization.storage(),
                current.register_environment(),
                candidate,
            )
        };
        assert!(validate(plan.clone()).is_ok(), "{target:?}");
        let index = layout
            .functions
            .iter()
            .position(|function| function.machine == caller)
            .unwrap();
        let mut omitted = plan.clone();
        omitted.functions[index].unwind.restores.pop();
        assert!(
            validate(omitted).is_err(),
            "{target:?}: an omitted restore is not canonical"
        );
        let mut invented = plan.clone();
        invented.functions[index]
            .unwind
            .restores
            .push(machine_code::FrameUnwindRestore {
                view: register_model::RegisterViewId(u16::MAX),
                frame_offset_bytes: 0,
                size_bytes: 8,
            });
        assert!(
            validate(invented).is_err(),
            "{target:?}: an invented restore is not canonical"
        );
        let mut reordered = plan.clone();
        reordered.functions[index].unwind.restores.reverse();
        if reordered.functions[index].unwind.restores != plan.functions[index].unwind.restores {
            assert!(
                validate(reordered).is_err(),
                "{target:?}: the canonical roster is ordered"
            );
        }
        let mut misattributed = plan.clone();
        misattributed.functions[index].unwind.restores[0].frame_offset_bytes += 8;
        assert!(
            validate(misattributed).is_err(),
            "{target:?}: a restore naming a different slot is not canonical"
        );
        let mut understated = plan.clone();
        understated.functions[index].unwind.released_bytes =
            plan.functions[index].unwind.released_bytes + 8;
        assert!(
            validate(understated).is_err(),
            "{target:?}: the release must be exactly the committed extent"
        );
        let mut custody = plan.clone();
        custody.functions[index].unwind.return_address =
            ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 0,
                size_bytes: 8,
            };
        assert!(
            validate(custody).is_err(),
            "{target:?}: the roster cannot restate different custody"
        );
        // A protocol whose epilogue bytes do not match the roster's
        // encoding fails replay even when it names the validated layout.
        let mut corrupted = protocol.clone();
        if encoding.epilogue.length != 0 {
            corrupted.bytes[usize::try_from(encoding.epilogue.offset).unwrap()] ^= 1;
            assert!(
                validate_target_frame_protocol_encoding(
                    realization.frame(),
                    current.register_environment(),
                    corrupted,
                )
                .is_err(),
                "{target:?}: epilogue bytes must encode the validated roster"
            );
        }
        // The same program still reaches ordinary callable publication on
        // every admitted target, unwind roster included.
        let emitted = stage_optimized_function_fragment_emission(
            physical.into_function_fragment_emission_source(),
        )
        .unwrap_or_else(|error| panic!("{target:?} emission: {error:?}"));
        let framed = stage_function_fragment_frame_application(emitted)
            .unwrap_or_else(|error| panic!("{target:?} frame: {error:?}"));
        let text = stage_optimized_fixed_frame_text_section(framed)
            .unwrap_or_else(|error| panic!("{target:?} text: {error:?}"));
        let object = stage_optimized_relocation_free_object_container(text)
            .unwrap_or_else(|error| panic!("{target:?} object: {error:?}"));
        let artifact = stage_validated_optimized_object_artifact(
            canonical_artifact(&semantic, &proof),
            object,
        )
        .unwrap_or_else(|error| panic!("{target:?} artifact: {error:?}"));
        let callable = stage_validated_optimized_ordinary_callable_entry(artifact)
            .unwrap_or_else(|error| panic!("{target:?} callable: {error:?}"));
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&callable).unwrap(),
            callable.custody(),
            "{target:?}"
        );
    }
}
