use super::*;
#[path = "stack_pointers/publication.rs"]
mod publication;

fn register_prefix(target: NativeTarget) -> usize {
    if target == NativeTarget::windows_x64() {
        4
    } else if target == NativeTarget::linux_x64() {
        6
    } else {
        8
    }
}

fn source(prefix: usize, incoming: bool, outgoing: bool) -> String {
    receiver_source(prefix, incoming, outgoing, "u16", "17", &[1])
}

fn receiver_source(
    prefix: usize,
    incoming: bool,
    outgoing: bool,
    scalar: &str,
    value: &str,
    receivers: &[usize],
) -> String {
    let parameters = (0..prefix)
        .map(|position| format!("argument{position}: u64"))
        .collect::<Vec<_>>()
        .join(", ");
    let receiver_parameters = if outgoing {
        format!(", {parameters}")
    } else {
        String::new()
    };
    let entry_parameters = if incoming {
        format!("{parameters}, ")
    } else {
        String::new()
    };
    let arguments = if outgoing {
        (0..prefix)
            .map(|position| {
                if incoming {
                    format!("argument{position}")
                } else {
                    format!("{}u64", position + 1)
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        String::new()
    };
    let calls = receivers
        .iter()
        .map(|receiver| format!("held[{receiver}].replace({arguments});"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "data Record [copy] {{ value: {scalar}; }}
        machine Record::replace(&write self{receiver_parameters}) {{ self.value = {value}; }}
        machine forward({entry_parameters}records: &write [Record; 2]) {{
            let held: &write [Record; 2] = &write records;
            {calls}
        }}"
    )
}

#[test]
fn stack_pointer_calls_preserve_runtime_scalar_values_and_root_across_calls() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let target = NativeTarget::host();
        let prefix = register_prefix(target);
        for position in 0..prefix {
            let source = receiver_source(
                prefix,
                true,
                true,
                "u64",
                &format!("argument{position}"),
                &[0, 1, 1],
            );
            let placed = native_text(&source, target);
            let module = terminal_codec::decode_module(artifact(&source).semantic_bytes()).unwrap();
            let text = placed.text_section();
            assert_eq!(text.resolved_internal_machine_calls.len(), 3);
            let entry = text
                .functions
                .iter()
                .find(|function| function.machine == module.entry)
                .unwrap();
            let parameters = "uint64_t, ".repeat(prefix);
            let scalar_bits = 0xfedc_ba98_7654_3200_u64;
            let arguments = (0..prefix)
                .map(|argument| format!("{}ull, ", scalar_bits + argument as u64))
                .collect::<String>();
            let expected = scalar_bits + position as u64;
            native_function::assert_c_text(
                &text.bytes,
                entry.section_offset.try_into().unwrap(),
                &format!(
                    r#"
                #include <stdint.h>
                #include <string.h>
                typedef struct {{ uint64_t value; }} Record;
                extern void omega_entry({parameters}Record *records);
                int main(void) {{
                    struct {{ uint64_t before; Record records[2]; uint64_t after; }} frame;
                    memset(&frame, 0xa5, sizeof frame);
                    frame.records[0].value = {expected}ull;
                    frame.records[1].value = {expected}ull;
                    unsigned char expected[sizeof frame];
                    memcpy(expected, &frame, sizeof frame);
                    frame.records[0].value = 0;
                    frame.records[1].value = 0;
                    omega_entry({arguments}frame.records);
                    if (memcmp(expected, &frame, sizeof frame)) return 1;
                    return 0;
                }}
            "#
                ),
            );
        }
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: runtime scalar stack-pointer calls require a supported Linux or macOS native host"
    );
}

#[test]
fn runtime_scalar_stack_pointer_calls_cross_lower_with_disjoint_spill_frames() {
    use selected_instructions::LocalStorageSlotId;

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let prefix = register_prefix(target);
        let source = receiver_source(
            prefix,
            true,
            true,
            "u64",
            &format!("argument{}", prefix - 1),
            &[0, 1, 1],
        );
        let placed = native_text(&source, target);
        assert_eq!(
            placed.text_section().resolved_internal_machine_calls.len(),
            3
        );
        let emission = placed.source().source().source();
        let entry = emission.selected_plan().entry;
        let frame = emission
            .frame_layout()
            .functions
            .iter()
            .find(|function| function.machine == entry)
            .unwrap();
        let spills = frame
            .local_storage_slots
            .iter()
            .filter(|slot| matches!(slot.id, LocalStorageSlotId::Spill { .. }))
            .collect::<Vec<_>>();
        if target.architecture == target::Architecture::Aarch64 {
            assert!(!spills.is_empty(), "nine durable values require a spill");
        }
        for (position, slot) in spills.iter().enumerate() {
            assert_eq!(slot.size_bytes, 8);
            assert_eq!(slot.alignment_bytes, 8);
            assert_eq!(slot.frame_offset_bytes % 8, 0);
            assert!(slot.frame_offset_bytes >= frame.outgoing_abi_area.byte_size);
            let end = slot.frame_offset_bytes + u64::from(slot.size_bytes);
            assert!(end <= frame.frame_size_bytes);
            for other in &spills[position + 1..] {
                assert_ne!(slot.id, other.id);
                let other_end = other.frame_offset_bytes + u64::from(other.size_bytes);
                assert!(end <= other.frame_offset_bytes || other_end <= slot.frame_offset_bytes);
            }
            for saved in &frame.callee_save_slots {
                let saved_end = saved.frame_offset_bytes + saved.size_bytes;
                assert!(end <= saved.frame_offset_bytes || saved_end <= slot.frame_offset_bytes);
            }
            if let machine_code::ReturnAddressFrameCustody::SavedLinkRegister {
                frame_offset_bytes,
                size_bytes,
                ..
            } = frame.return_address
            {
                assert!(
                    end <= frame_offset_bytes
                        || frame_offset_bytes + u64::from(size_bytes) <= slot.frame_offset_bytes
                );
            }
        }
    }
}

#[test]
fn spill_realization_rejects_changed_allocation_under_retained_frame_evidence() {
    use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedInstructionKind};

    let source = receiver_source(8, true, true, "u64", "argument7", &[0, 1, 1]);
    let artifact = artifact(&source);
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimize(&artifact),
            NativeTarget::macos_arm64(),
            &[],
        )
        .unwrap();
    let mut framed = physical.into_fixed_frame_for_test();
    machine_emission::validate_fixed_frame_function_relative_realization(&framed).unwrap();
    let original = framed.allocation().program().clone();
    for mutation in 0..3 {
        let mut changed = original.clone();
        let selected = std::sync::Arc::make_mut(&mut changed.selected);
        let entry = selected.entry;
        let caller = selected
            .functions
            .iter_mut()
            .find(|function| function.machine == entry)
            .unwrap();
        let slot = caller
            .local_storage_slots
            .iter_mut()
            .find(|slot| matches!(slot.id, LocalStorageSlotId::Spill { .. }))
            .unwrap();
        match mutation {
            0 => slot.byte_size = 16,
            1 => slot.alignment = 4,
            2 => {
                let store = caller
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.instructions)
                    .find(|instruction| {
                        matches!(
                            instruction.kind,
                            SelectedInstructionKind::Store64 {
                                slot: FrameStorageSlotId::Local(LocalStorageSlotId::Spill { .. }),
                                ..
                            }
                        )
                    })
                    .unwrap();
                let SelectedInstructionKind::Store64 { byte_offset, .. } = &mut store.kind else {
                    unreachable!()
                };
                *byte_offset = 8;
            }
            _ => unreachable!(),
        }
        framed
            .allocation_mut()
            .substitute_current_program_for_test(changed);
        assert!(
            machine_emission::validate_fixed_frame_function_relative_realization(&framed).is_err(),
            "stale frame accepted allocation mutation {mutation}"
        );
    }
    framed
        .allocation_mut()
        .substitute_current_program_for_test(original);
    machine_emission::validate_fixed_frame_function_relative_realization(&framed).unwrap();
}

#[test]
fn projected_receiver_stack_pointers_cross_lower() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for (incoming, outgoing) in [(true, false), (false, true), (true, true)] {
            let source = source(register_prefix(target), incoming, outgoing);
            let placed = native_text(&source, target);
            assert_eq!(placed.text_section().functions.len(), 2);
            assert_eq!(
                placed.text_section().resolved_internal_machine_calls.len(),
                1
            );
        }
    }
}

#[test]
fn projected_receiver_stack_pointers_update_original_caller_storage() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    for (incoming, outgoing) in [(true, false), (false, true), (true, true)] {
        let target = NativeTarget::host();
        let prefix = register_prefix(target);
        let source = source(prefix, incoming, outgoing);
        let placed = native_text(&source, target);
        let module = terminal_codec::decode_module(artifact(&source).semantic_bytes()).unwrap();
        let text = placed.text_section();
        let entry = text
            .functions
            .iter()
            .find(|function| function.machine == module.entry)
            .unwrap();
        let parameters = if incoming {
            "uint64_t, ".repeat(prefix)
        } else {
            String::new()
        };
        let arguments = if incoming {
            (0..prefix)
                .map(|position| format!("{}ull, ", position + 1))
                .collect::<String>()
        } else {
            String::new()
        };
        native_function::assert_c_text(
            &text.bytes,
            entry.section_offset.try_into().unwrap(),
            &format!(
                r#"
            #include <stdint.h>
            #include <string.h>
            typedef struct {{ uint16_t value; }} Record;
            extern void omega_entry({parameters}Record *records);
            int main(void) {{
                struct {{ uint64_t before; Record records[2]; uint64_t after; }} frame;
                memset(&frame, 0xa5, sizeof frame);
                for (unsigned iteration = 0; iteration < 3; ++iteration) {{
                    frame.records[1].value = 17;
                    unsigned char expected[sizeof frame];
                    memcpy(expected, &frame, sizeof frame);
                    frame.records[1].value = iteration;
                    omega_entry({arguments}frame.records);
                    if (memcmp(expected, &frame, sizeof frame)) return 1;
                }}
                return 0;
            }}
        "#
            ),
        );
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: stack-pointer caller observations require a supported Linux or macOS native host"
    );
}
