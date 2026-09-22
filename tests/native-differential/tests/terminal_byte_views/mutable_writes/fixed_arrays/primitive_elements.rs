//! Ordinary scalar subloans must address the caller's actual array element.

use super::super::super::byte_view_target;
#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
use super::native_function;
use super::{NativeTarget, lower_writer, publish_lowered};
const SOURCE: &str = r#"
data Record { before: u8; bytes: [u8; 3]; after: u64; }
machine replace(destination: &write u8, value: u8) { destination = value; }
machine observe(input: &u8, output: &mut u8) { output = input; }
machine Record::run(&mut self, output: &mut u8, value: u8) {
    replace(&write self.bytes[1], value);
    observe(&self.bytes[1], output);
}
"#;

#[test]
fn primitive_element_source_rejects_out_of_bounds_and_write_only_observation() {
    for source in [
        SOURCE.replace("bytes[1]", "bytes[3]"),
        SOURCE.replace("input: &u8", "input: &write u8"),
    ] {
        assert!(
            crate::front_end::checked_program_result(&source).is_err(),
            "invalid element access must not reach Terminal production"
        );
    }
}

#[test]
fn primitive_element_native_replay_rejects_redirected_paths_and_access() {
    let lowered = lower_writer(SOURCE, "Record::run");
    for target in [
        NativeTarget::macos_arm64(),
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let compiled = byte_view_target(&lowered.semantic_module, &lowered.proof_bundle, target);
        let abstracted = compiled.optimized().plan();
        let unit = compiled.optimized();
        target_operations_to_selected_instructions::legalize_target_operations(
            compiled.target_operations(),
            abstracted,
            unit,
        )
        .unwrap();
        for mutation in 0..4 {
            let mut changed = compiled.target_operations().clone();
            let caller = changed
                .functions
                .iter_mut()
                .find(|function| function.machine == lowered.semantic_module.entry)
                .unwrap();
            let argument = caller
                .graph
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| {
                    let target_operations::TargetUnitOperation::Call { arguments, .. } = operation
                    else {
                        return None;
                    };
                    arguments.iter_mut().find(|argument| {
                        argument.access == terminal_psi::StructuralAccess::SharedBorrow
                            && !argument.path.is_empty()
                    })
                })
                .unwrap();
            match mutation {
                0 => {
                    // A coherent offset for another same-typed element still
                    // cannot replace the source's independently retained path.
                    *argument.path.last_mut().unwrap() =
                        terminal_psi::StructuralPathSegment::FixedIndex(2);
                    argument.source_byte_offset += 1;
                }
                1 => {
                    *argument.path.last_mut().unwrap() =
                        terminal_psi::StructuralPathSegment::FixedIndex(3);
                    argument.source_byte_offset += 2;
                }
                2 => argument.access = terminal_psi::StructuralAccess::MutableBorrow,
                3 => {
                    argument.fixed_array_length = Some(3);
                    argument.element_stride = Some(1);
                }
                _ => unreachable!(),
            }
            assert!(
                abstract_operations_to_target_operations::validate_abstract_to_target_translation(
                    abstracted, target, &changed
                )
                .is_err(),
                "forged primitive path {mutation} on {target:?}"
            );
            assert!(
                target_operations_to_selected_instructions::legalize_target_operations(
                    &changed, abstracted, unit
                )
                .is_err(),
                "forged primitive path legalization {mutation} on {target:?}"
            );
        }
    }
}

#[test]
fn nested_primitive_element_loans_reconstruct_stride_and_field_offsets() {
    let source = r#"
        data Row { before: u8; bytes: [u32; 3]; after: u8; }
        data Record { before: u8; rows: [Row; 2]; after: u64; }
        machine replace(destination: &mut u32, value: u32) { destination = value; }
        machine observe(input: &u32, output: &mut u32) { output = input; }
        machine Record::run(&mut self, output: &mut u32, value: u32) {
            let initial: u32 = value;
            replace(&mut self.rows[1].bytes[2], initial);
            observe(&self.rows[1].bytes[2], output);
        }
    "#;
    let lowered = lower_writer(source, "Record::run");
    for target in [
        NativeTarget::macos_arm64(),
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (image, entry_offset) = publish_lowered(target, &lowered);
        if target != NativeTarget::host() {
            continue;
        }
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry_offset,
            r#"
            #include <stdint.h>
            #include <string.h>
            #include <unistd.h>
            struct Row { uint8_t before; uint32_t bytes[3]; uint8_t after; };
            struct Record { uint8_t before; struct Row rows[2]; uint64_t after; };
            extern void omega_entry(uint32_t value, struct Record *record, uint32_t *output);
            int main(void) {
                alarm(10);
                struct Record actual, expected;
                memset(&actual, 0x5a, sizeof(actual));
                uint32_t values[] = {0, 1, 0x80000000, 0xffffffff};
                for (unsigned position = 0; position < 4; ++position) {
                    memcpy(&expected, &actual, sizeof(actual));
                    expected.rows[1].bytes[2] = values[position];
                    uint32_t output = 0xa5a5a5a5;
                    omega_entry(values[position], &actual, &output);
                    if (output != values[position] || memcmp(&actual, &expected, sizeof(actual))) return 1;
                }
                return 0;
            }
        "#,
        );
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        {
            let _ = (image, entry_offset);
            eprintln!(
                "SKIP: nested primitive element execution needs a matching supported native C/text host"
            );
        }
    }
}

#[test]
fn primitive_element_loans_publish_and_mutate_original_array_storage() {
    let lowered = lower_writer(SOURCE, "Record::run");
    for target in [
        NativeTarget::macos_arm64(),
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (image, entry_offset) = publish_lowered(target, &lowered);
        if target != NativeTarget::host() {
            continue;
        }
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry_offset,
            r#"
            #include <stdint.h>
            #include <string.h>
            #include <unistd.h>
            struct Record { uint8_t before; uint8_t bytes[3]; uint64_t after; };
            extern void omega_entry(uint8_t value, struct Record *record, uint8_t *output);
            int main(void) {
                alarm(10);
                struct Record actual, expected;
                memset(&actual, 0x5a, sizeof(actual));
                for (unsigned value = 0; value <= 255; ++value) {
                    memcpy(&expected, &actual, sizeof(actual));
                    expected.bytes[1] = value;
                    uint8_t output = 0xa5;
                    omega_entry(value, &actual, &output);
                    if (output != value || memcmp(&actual, &expected, sizeof(actual))) return 1;
                }
                return 0;
            }
        "#,
        );
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        {
            let _ = (image, entry_offset);
            eprintln!(
                "SKIP: primitive element execution needs a matching supported native C/text host"
            );
        }
    }
}
