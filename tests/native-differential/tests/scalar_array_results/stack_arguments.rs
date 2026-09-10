//! Owned array payloads remain real values after argument registers are exhausted.
use super::*;

#[test]
fn outgoing_stack_array_replay_binds_argument_extent_and_exact_writes() {
    use selected_instructions::{SelectedInstructionKind, SelectedMemoryAccessRole};
    let source = "
        machine keep(p0: u64, p1: u64, p2: u64, p3: u64,
                     p4: u64, p5: u64, p6: u64, p7: u64, row: [u8; 11]) -> [u8; 11] { row }
        machine selected(value: u8) -> [u8; 11] {
            let row: [u8; 11] = [value, 1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8, 8u8, 9u8, 10u8];
            keep(0u64, 1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64, row)
        }
    ";
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let staged =
            target_operations_to_selected_instructions::stage_optimized_instruction_selection(
                target_plan(source, "selected", target).unwrap(),
                environment,
            )
            .unwrap();
        let environment = staged.register_environment();
        let constraints = target_operations_to_selected_instructions::selection_constraints(
            staged.legalized(),
            environment,
        );
        let plan = staged.selected().plan();
        let function_index = plan
            .functions
            .iter()
            .position(|function| {
                function
                    .outgoing_arguments
                    .iter()
                    .any(|slot| slot.byte_size == 11)
            })
            .unwrap();
        let function = &plan.functions[function_index];
        let slot_index = function
            .outgoing_arguments
            .iter()
            .position(|slot| slot.byte_size == 11)
            .unwrap();
        let slot = function.outgoing_arguments[slot_index].id;
        let writes = function.memory_accesses.iter().filter(|access|
            matches!(access.role, SelectedMemoryAccessRole::WriteOutgoing { slot: destination } if destination == slot)).collect::<Vec<_>>();
        assert_eq!(writes.len(), 2);
        assert_eq!(
            writes.iter().map(|access| access.byte_count).sum::<u32>(),
            11
        );
        for mutation in 0..7 {
            let mut changed = plan.clone();
            let function = &mut changed.functions[function_index];
            match mutation {
                0 => function.outgoing_arguments[slot_index].abi_stack_byte_offset += 8,
                1 => function.outgoing_arguments[slot_index].byte_size += 1,
                2 => function.outgoing_arguments[slot_index].id.argument_index += 1,
                3 => function.outgoing_arguments[slot_index].alignment = 1,
                4 => {
                    let write = function.memory_accesses.iter_mut().find(|access|
                        matches!(access.role, SelectedMemoryAccessRole::WriteOutgoing { slot: destination } if destination == slot)).unwrap();
                    write.byte_offset += 1;
                }
                5 => {
                    let store = function
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.instructions)
                        .find(|row| matches!(row.kind, SelectedInstructionKind::StorePacked { .. }))
                        .unwrap();
                    let SelectedInstructionKind::StorePacked { width, .. } = &mut store.kind else {
                        unreachable!();
                    };
                    *width = selected_instructions::PackedByteWidth::Five;
                }
                6 => {
                    function.outgoing_arguments.remove(slot_index);
                }
                _ => unreachable!(),
            }
            assert!(
                target_operations_to_selected_instructions::validate_selected_instructions(
                    staged.legalized(),
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed,
                )
                .is_err(),
                "{target:?} mutation {mutation}"
            );
        }
    }
}

fn mixed_call_source(length: usize) -> String {
    // The scalar-result checked graph still rejects named array locals. Keep
    // that source-materialization gap on TASKS; this literal reaches the ABI
    // transport without suggesting that local spelling is unsupported semantics.
    let leaves = (1..length)
        .map(|value| format!("{value}u8"))
        .chain(["value".to_owned()])
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "
        machine write(row: [u8; {length}], output: &mut u8) {{ output = 201u8; }}
        machine consume(row: [u8; {length}], value: u8) -> u8 {{ value }}
        machine forward(row: [u8; {length}], output: &mut u8, value: u8) -> u8 {{
            write(row, output);
            consume(row, value)
        }}
        machine selected(output: &mut u8, value: u8) -> u8 {{
            forward([{leaves}], output, value)
        }}
    "
    )
}

#[test]
fn large_sysv_inline_arguments_retain_selected_call_and_stack_evidence() {
    // SysV passes this value inline on the stack. AAPCS and Microsoft use
    // indirect arguments at this size, an explicitly separate transport.
    // This is deliberately selected-stage coverage, not native acceptance:
    // the literal's multi-block graph needs runtime spill support (TASKS).
    let target = NativeTarget::linux_x64();
    let staged = target_operations_to_selected_instructions::stage_optimized_instruction_selection(
        target_plan(&mixed_call_source(17), "selected", target).unwrap(),
        register_environment::baseline_target_register_environment(target).unwrap(),
    )
    .unwrap();
    let plan = staged.selected().plan();
    assert!(plan.functions.iter().any(|function| {
        function
            .outgoing_arguments
            .iter()
            .any(|slot| slot.byte_size == 17)
    }));
    assert!(
        plan.functions
            .iter()
            .any(|function| function.calls.iter().any(|call| call
                .call
                .arguments
                .iter()
                .filter(|argument| matches!(
                    argument,
                    legalized_operations::LegalizedScalarArgument::Structural { .. }
                ))
                .count()
                == 2))
    );
    let environment = staged.register_environment();
    let constraints = target_operations_to_selected_instructions::selection_constraints(
        staged.legalized(),
        environment,
    );
    for function_index in 0..plan.functions.len() {
        for call_index in 0..plan.functions[function_index].calls.len() {
            let mut changed = plan.clone();
            let arguments = &mut changed.functions[function_index].calls[call_index]
                .call
                .arguments;
            if arguments.len() < 2 {
                continue;
            }
            arguments.swap(0, 1);
            assert!(
                target_operations_to_selected_instructions::validate_selected_instructions(
                    staged.legalized(),
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed,
                )
                .is_err()
            );
        }
    }
}

#[test]
fn large_sysv_inline_arguments_publish_through_block_local_spills() {
    let (image, offset) = publish(
        &mixed_call_source(17),
        "selected",
        NativeTarget::linux_x64(),
    );
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    native_function::assert_c_text(
        &image.output().final_text_bytes,
        offset,
        "#include <stdint.h>
         extern uint8_t omega_entry(uint8_t, uint8_t*);
         int main(void) {
             for (unsigned value = 0; value < 256; ++value) {
                 uint8_t output = 0;
                 if (omega_entry(value, &output) != value || output != 201) return 1;
             }
             return 0;
         }",
    );
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (image, offset);
        eprintln!("SKIP: 17-byte SysV runtime requires a Linux x86-64 host");
    }
}

#[test]
fn owned_arrays_compose_with_borrowed_outputs_and_scalar_results() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (image, offset) = publish(&mixed_call_source(8), "selected", target);
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        if target == NativeTarget::host() {
            native_function::assert_c_text(
                &image.output().final_text_bytes,
                offset,
                "#include <stdint.h>
         extern uint8_t omega_entry(uint8_t, uint8_t*);
         int main(void) {
             for (unsigned value = 0; value < 256; ++value) {
                 uint8_t output = 0;
                 if (omega_entry(value, &output) != value || output != 201) return 1;
             }
             return 0;
         }",
            );
        }
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        {
            let _ = (image, offset);
            eprintln!("SKIP: mixed-array runtime requires the Linux/macOS host harness");
        }
    }
}

#[test]
fn stack_array_arguments_preserve_scalar_width_fragments() {
    check_stack_lengths(&[1, 2, 4]);
}

#[test]
fn stack_array_arguments_preserve_packed_single_fragments() {
    check_stack_lengths(&[3, 5, 6, 7]);
}

#[test]
fn stack_array_arguments_preserve_packed_second_fragments() {
    check_stack_lengths(&[9, 11, 13, 15]);
}

#[test]
fn stack_array_arguments_preserve_two_complete_fragments() {
    check_stack_lengths(&[16]);
}

fn check_stack_lengths(lengths: &[usize]) {
    for &length in lengths {
        let leaves = (0..length)
            .map(|position| {
                if position == 0 {
                    "value".to_owned()
                } else {
                    format!("{}u8", (position * 37 + 129) % 256)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!(
            "machine keep(p0: u64, p1: u64, p2: u64, p3: u64,
                          p4: u64, p5: u64, p6: u64, p7: u64, row: [u8; {length}]) -> [u8; {length}] {{ row }}
             machine make(value: u8) -> [u8; {length}] {{ [{leaves}] }}
             machine selected(value: u8) -> [u8; {length}] {{
                 let row: [u8; {length}] = make(value);
                 keep(0u64, 1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64, row)
             }}"
        );
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            if target == NativeTarget::windows_x64() && !matches!(length, 1 | 2 | 4 | 8) {
                continue; // Microsoft requires the separate indirect transport at these sizes.
            }
            let (image, offset) = publish(&source, "selected", target);
            #[cfg(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            ))]
            if target == NativeTarget::host() {
                let (result_type, fragments) = if length <= 8 {
                    ("uint64_t", "uint64_t fragments[2] = {result, 0};")
                } else {
                    (
                        "Pair",
                        "uint64_t fragments[2] = {result.first, result.second};",
                    )
                };
                native_function::assert_c_text(&image.output().final_text_bytes, offset, &format!(
                    "#include <stdint.h>\n
                     typedef struct {{ uint64_t first; uint64_t second; }} Pair;
                     extern {result_type} omega_entry(uint8_t);
                     int main(void) {{
                         for (unsigned value = 0; value < 256; ++value) {{
                             {result_type} result = omega_entry(value);
                             {fragments}
                             for (unsigned position = 0; position < {length}; ++position) {{
                                 uint8_t expected = position == 0 ? value : (position * 37 + 129) % 256;
                                 uint8_t actual = fragments[position / 8] >> ((position % 8) * 8);
                                 if (actual != expected) return 1;
                             }}
                         }}
                         return 0;
                     }}"
                ));
            }
            #[cfg(not(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            )))]
            {
                let _ = (image, offset);
                eprintln!("SKIP: stack-array runtime requires the Linux/macOS host harness");
            }
        }
    }
}

#[test]
fn owned_array_arguments_cross_exhausted_register_banks() {
    let source = "
        machine keep(p0: u64, p1: u64, p2: u64, p3: u64,
                     p4: u64, p5: u64, p6: u64, p7: u64, row: [u8; 8]) -> [u8; 8] {
            row
        }
        machine forward(p0: u64, p1: u64, p2: u64, p3: u64,
                        p4: u64, p5: u64, p6: u64, p7: u64, row: [u8; 8]) -> [u8; 8] {
            keep(p0, p1, p2, p3, p4, p5, p6, p7, row)
        }
        machine selected(value: u8) -> [u8; 8] {
            let row: [u8; 8] = [value, 9u8, 17u8, 33u8, 65u8, 129u8, 201u8, 255u8];
            forward(0u64, 1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64, row)
        }
    ";
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (image, offset) = publish(source, "selected", target);
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        if target == NativeTarget::host() {
            native_function::assert_c_text(
                &image.output().final_text_bytes,
                offset,
                "#include <stdint.h>
                 typedef struct { uint8_t values[8]; } Row;
                 extern Row omega_entry(uint8_t);
                 int main(void) {
                     const uint8_t tail[] = {9, 17, 33, 65, 129, 201, 255};
                     for (unsigned value = 0; value < 256; ++value) {
                         Row row = omega_entry(value);
                         if (row.values[0] != value) return 1;
                         for (unsigned element = 1; element < 8; ++element)
                             if (row.values[element] != tail[element - 1]) return 2;
                     }
                     return 0;
                 }",
            );
        }
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        {
            let _ = (image, offset);
            eprintln!("SKIP: stack-array runtime requires the Linux/macOS host harness");
        }
    }
}
