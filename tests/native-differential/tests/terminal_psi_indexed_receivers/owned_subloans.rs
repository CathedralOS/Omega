//! Owned callers observe writes through projected exclusive subloans.

use super::{NativeTarget, native_function, native_text, primitive_stores};

fn source(access: &str, record_first: bool) -> String {
    let fields = if record_first {
        "record: Record; untouched: u64;"
    } else {
        "untouched: u64; record: Record;"
    };
    format!(
        "data Record [copy] {{ value: u64; }}
         data Container [copy] {{ {fields} }}
         machine Record::replace(&{access} self, value: u64) {{ self.value = value; }}
         machine update(mut container: Container, value: u64, output: &mut u64, untouched: &mut u64) {{
             container.record.replace(value);
             output = container.record.value;
             untouched = container.untouched;
         }}
         machine forward(value: u64, output: &mut u64, untouched: &mut u64) {{
             let container: Container = Container {{ record: Record {{ value: 201 }}, untouched: 99 }};
             update(container, value, output, untouched);
         }}"
    )
}

#[test]
fn owned_record_field_subloans_restore_caller_visible_writes() {
    for (access, record_first) in [
        ("mut", true),
        ("write", true),
        ("mut", false),
        ("write", false),
    ] {
        let source = source(access, record_first);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let (bytes, entry) = primitive_stores::published_text(&source, target);
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
                &bytes,
                entry,
                r#"
                #include <stdint.h>
                typedef struct { uint64_t value; uint64_t untouched; } Container;
                extern void omega_entry(uint64_t value, uint64_t *output, uint64_t *untouched);
                int main(void) {
                    struct { uint64_t before; Container output; uint64_t after; } frame =
                        { 0x1234, {0, 0}, 0x5678 };
                    omega_entry(41, &frame.output.value, &frame.output.untouched);
                    if (frame.output.value != 41 || frame.output.untouched != 99 ||
                        frame.before != 0x1234 || frame.after != 0x5678) return 1;
                    omega_entry(77, &frame.output.value, &frame.output.untouched);
                    return frame.output.value != 77 || frame.output.untouched != 99 ||
                        frame.before != 0x1234 || frame.after != 0x5678;
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
                let _ = (bytes, entry);
                eprintln!(
                    "SKIP: owned projected subloan execution requires a supported Linux or macOS host"
                );
            }
        }
    }
}

#[test]
fn owned_field_publication_rejects_substituted_projection_and_backing() {
    // This ABI retains the owned aggregate's incoming indirect home. The
    // execution test also covers direct register aggregates on the other ABIs.
    let placed = native_text(&source("mut", false), NativeTarget::windows_x64());
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(placed).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    let caller = object
        .functions()
        .iter()
        .find(|function| {
            function.internal_unit_calls.iter().any(|call| {
                call.arguments
                    .iter()
                    .any(|argument| !argument.path.is_empty())
            })
        })
        .unwrap();
    let machine = caller.machine;
    let call_position = caller
        .internal_unit_calls
        .iter()
        .position(|call| {
            call.arguments
                .iter()
                .any(|argument| !argument.path.is_empty())
        })
        .unwrap();
    let argument_position = caller.internal_unit_calls[call_position]
        .arguments
        .iter()
        .position(|argument| !argument.path.is_empty())
        .unwrap();
    let argument = &caller.internal_unit_calls[call_position].arguments[argument_position];
    assert_eq!(argument.source_byte_offset, 8);
    assert!(caller.unit_parameter_homes.iter().any(|home| {
        home.place == argument.place && home.access == terminal_psi::StructuralAccess::Owned
    }));
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    for mutation in 0..9 {
        let mutate = |argument: &mut machine_code::InternalUnitCallArgumentRecord| match mutation {
            0 => argument.source_byte_offset = 0,
            1 => argument.path.clear(),
            2 => {
                argument.source_location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 0 }
            }
            3 => argument.access = terminal_psi::StructuralAccess::Owned,
            4 => argument.bytes[0] ^= 1,
            5 => argument.root_structural_type = argument.structural_type,
            6 => argument.destination.locations.clear(),
            7 => argument.fixed_array_length = Some(8),
            8 => argument.structural_type = argument.root_structural_type,
            _ => unreachable!(),
        };
        let mut changed = object.clone();
        let caller = changed
            .functions_mut_for_test()
            .iter_mut()
            .find(|function| function.machine == machine)
            .unwrap();
        mutate(&mut caller.internal_unit_calls[call_position].arguments[argument_position]);
        assert!(
            image_emission::validate_function_fragment_object_artifact(&source, &changed).is_err(),
            "object mutation {mutation}"
        );
        assert!(
            image_emission::emit_executable_image(&changed, 3).is_err(),
            "image mutation {mutation}"
        );

        let mut changed = record.clone();
        let call = changed
            .internal_unit_calls_mut_for_test()
            .iter_mut()
            .filter(|call| call.machine == machine)
            .nth(call_position)
            .unwrap();
        mutate(&mut call.custody.arguments[argument_position]);
        let encoded = image_emission::encode_installation_record(&changed);
        // A plausible aligned offset or retained byte string may be serialized,
        // but neither can replace the independently admitted image's custody.
        if matches!(mutation, 0 | 4) {
            assert!(encoded.is_ok(), "shape mutation {mutation}: {encoded:?}");
        }
        if let Ok(encoded) = encoded {
            let decoded = image_emission::decode_installation_record(&encoded).unwrap();
            assert!(
                image_emission::validate_installation_record(&decoded, &image).is_err(),
                "decoded mutation {mutation}"
            );
        }
        assert!(
            image_emission::validate_installation_record(&changed, &image).is_err(),
            "record mutation {mutation}"
        );
    }
}
