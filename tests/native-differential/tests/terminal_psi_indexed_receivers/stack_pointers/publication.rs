//! Publication retains borrowed pointer transport and the realized spill frame.
use super::*;
use machine_code::StructuralSourceLocation;

fn container(
    target: NativeTarget,
    source: &str,
) -> std::sync::Arc<object_file::StagedOptimizedRelocationFreeObjectContainer> {
    std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(native_text(source, target))
            .unwrap(),
    )
}

fn pressure_source(target: NativeTarget) -> String {
    let prefix = register_prefix(target);
    receiver_source(
        prefix,
        true,
        true,
        "u64",
        &format!("argument{}", prefix - 1),
        &[0, 1, 1],
    )
}

#[test]
fn stack_pointer_calls_publish_objects_and_installation_records() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let source = container(target, &pressure_source(target));
        let object = image_emission::build_function_fragment_object_artifact(source.clone())
            .unwrap_or_else(|error| panic!("borrowed object on {target:?}: {error}"));
        image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
        let text = source.source().text_section();
        assert_eq!(object.text_bytes(), text.bytes);
        let caller = object
            .functions()
            .iter()
            .find(|function| function.machine == text.semantic_entry)
            .unwrap();
        assert_eq!(caller.internal_unit_calls.len(), 3);
        assert!(matches!(
            caller.unit_parameter_homes[0].location,
            StructuralSourceLocation::IncomingBorrowedPointer {
                location: calling_conventions::IndirectPointerLocation::Stack { .. }
            }
        ));
        for call in &caller.internal_unit_calls {
            assert_eq!(call.scalar_arguments.len(), register_prefix(target));
            assert_eq!(call.arguments.len(), 1);
            let argument = &call.arguments[0];
            assert_eq!(argument.code_offset, call.code_offset);
            assert_eq!(argument.byte_count, call.byte_count);
            assert_eq!(argument.bytes.len(), call.byte_count);
            assert_eq!(
                argument.access,
                terminal_psi::StructuralAccess::WriteOnlyBorrow
            );
            assert_eq!(
                argument.call_stack_bytes,
                caller.unit_stack.unwrap().frame_bytes
            );
        }
        let image = image_emission::emit_executable_image(&object, 3).unwrap();
        image_emission::validate_executable_image(&object, &image).unwrap();
        let record = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .unwrap();
        let encoded = image_emission::encode_installation_record(&record).unwrap();
        let decoded = image_emission::decode_installation_record(&encoded).unwrap();
        image_emission::validate_installation_record(&decoded, &image).unwrap();
        let demand = image_emission::derive_stack_demand(&object, text.semantic_entry).unwrap();
        assert_eq!(
            demand,
            image_emission::derive_installation_stack_demand(&decoded, &image, text.semantic_entry)
                .unwrap()
        );
        let callee = object
            .functions()
            .iter()
            .find(|function| function.machine == caller.internal_unit_calls[0].target)
            .unwrap();
        assert_eq!(
            demand.ceiling_bytes(),
            u64::from(
                caller.unit_call_stacks[0].caller_live_bytes
                    + callee.unit_stack.unwrap().local_peak_bytes
            )
        );
        let mut stale_frame = decoded.clone();
        stale_frame
            .functions_mut_for_test()
            .iter_mut()
            .find(|function| function.machine == text.semantic_entry)
            .unwrap()
            .unit_stack
            .as_mut()
            .unwrap()
            .frame_bytes += 16;
        assert!(
            image_emission::derive_installation_stack_demand(
                &stale_frame,
                &image,
                text.semantic_entry
            )
            .is_err()
        );
        if target == NativeTarget::host() {
            assert_published_caller(
                &image.output().final_text_bytes,
                caller.text_offset,
                register_prefix(target),
            );
        }
    }
}

fn assert_published_caller(bytes: &[u8], entry_offset: usize, prefix: usize) {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let scalar_bits = 0xfedc_ba98_7654_3200_u64;
        let parameters = "uint64_t, ".repeat(prefix);
        let arguments = (0..prefix)
            .map(|position| format!("{}ull, ", scalar_bits + position as u64))
            .collect::<String>();
        let expected = scalar_bits + prefix as u64 - 1;
        native_function::assert_c_text(
            bytes,
            entry_offset,
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
                return memcmp(expected, &frame, sizeof frame) != 0;
            }}
        "#
            ),
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
        let _ = (bytes, entry_offset, prefix);
        eprintln!(
            "SKIP: published borrowed caller requires a supported Linux or macOS native host"
        );
    }
}

#[test]
fn borrowed_publication_rejects_substituted_roots_projections_and_transport() {
    let target = NativeTarget::macos_arm64();
    let source = container(target, &pressure_source(target));
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    let entry = source.source().text_section().semantic_entry;
    for mutation in 0..12 {
        let mut changed = object.clone();
        let caller = changed
            .functions_mut_for_test()
            .iter_mut()
            .find(|function| function.machine == entry)
            .unwrap();
        match mutation {
            0 => {
                caller.unit_parameter_homes[0].location =
                    StructuralSourceLocation::Stack { byte_offset: 0 }
            }
            1 => {
                caller.unit_parameter_homes[0].access =
                    terminal_psi::StructuralAccess::MutableBorrow
            }
            2 => caller.internal_unit_calls.swap(0, 1),
            3 => caller.internal_unit_calls[0].arguments[0].source_byte_offset += 8,
            4 => caller.internal_unit_calls[0].arguments[0].path.clear(),
            5 => {
                caller.internal_unit_calls[0].arguments[0].access =
                    terminal_psi::StructuralAccess::Owned
            }
            6 => caller.internal_unit_calls[0].arguments[0].bytes[0] ^= 1,
            7 => caller.internal_unit_calls[0].arguments[0].call_stack_bytes += 16,
            8 => {
                caller.internal_unit_calls[0].arguments[0].source_location =
                    StructuralSourceLocation::IncomingBorrowedPointer {
                        location: calling_conventions::IndirectPointerLocation::Stack {
                            stack_byte_offset: 8,
                            alignment: 8,
                        },
                    }
            }
            9 => caller.internal_unit_calls[0].arguments[0]
                .destination
                .locations
                .clear(),
            10 => caller.internal_unit_calls[0].arguments[0].code_offset += 4,
            11 => caller.internal_unit_calls[0].scalar_arguments[0].parameter_index += 1,
            _ => unreachable!(),
        }
        assert!(
            image_emission::validate_function_fragment_object_artifact(&source, &changed).is_err(),
            "mutation {mutation}"
        );
        assert!(
            image_emission::emit_executable_image(&changed, 3).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn register_and_stack_pointer_directions_publish_without_referent_copies() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for (incoming, outgoing) in [(false, false), (false, true), (true, false)] {
            let source = container(target, &source(register_prefix(target), incoming, outgoing));
            let object = image_emission::build_function_fragment_object_artifact(source).unwrap();
            let image = image_emission::emit_executable_image(&object, 3).unwrap();
            let record = image_emission::build_installation_record(
                &image,
                semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
            )
            .unwrap_or_else(|error| {
                panic!("{target:?} incoming={incoming} outgoing={outgoing}: {error}")
            });
            let encoded = image_emission::encode_installation_record(&record).unwrap();
            let decoded = image_emission::decode_installation_record(&encoded).unwrap();
            image_emission::validate_installation_record(&decoded, &image).unwrap();
        }
    }
}

#[test]
fn borrowed_installation_requires_exact_image_custody() {
    let target = NativeTarget::macos_arm64();
    let source = container(target, &pressure_source(target));
    let entry = source.source().text_section().semantic_entry;
    let object = image_emission::build_function_fragment_object_artifact(source).unwrap();
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    for mutation in 0..4 {
        let mut changed = record.clone();
        if mutation == 3 {
            let caller = changed
                .functions_mut_for_test()
                .iter_mut()
                .find(|function| function.machine == entry)
                .unwrap();
            let stack = caller.unit_stack.as_mut().unwrap();
            stack.frame_bytes += 16;
            stack.local_peak_bytes += 16;
            for call in &mut caller.unit_call_stacks {
                call.active_frame_bytes += 16;
                call.caller_live_bytes += 16;
            }
            for call in changed
                .internal_unit_calls_mut_for_test()
                .iter_mut()
                .filter(|call| call.machine == entry)
            {
                for argument in &mut call.custody.arguments {
                    argument.call_stack_bytes += 16;
                }
            }
        } else {
            let calls = changed.internal_unit_calls_mut_for_test();
            match mutation {
                0 => {
                    let path = calls[1].custody.arguments[0].path.clone();
                    let source_byte_offset = calls[1].custody.arguments[0].source_byte_offset;
                    calls[0].custody.arguments[0].path = path;
                    calls[0].custody.arguments[0].source_byte_offset = source_byte_offset;
                }
                1 => calls[0].custody.arguments[0].bytes[0] ^= 1,
                2 => {
                    let replacement = calls[0].custody.scalar_arguments[1].source.source_value();
                    let machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
                        source_value,
                        ..
                    } = &mut calls[0].custody.scalar_arguments[0].source
                    else {
                        panic!("selected scalar source");
                    };
                    *source_value = replacement;
                }
                _ => unreachable!(),
            }
        }
        // Plausible record geometry is not authority to replace the admitted
        // projection, scalar value, call bytes, or actual spill frame.
        let encoded = image_emission::encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("shape mutation {mutation}: {error}"));
        let decoded = image_emission::decode_installation_record(&encoded).unwrap();
        assert!(
            image_emission::validate_installation_record(&decoded, &image).is_err(),
            "mutation {mutation}"
        );
        assert!(
            image_emission::derive_installation_stack_demand(&decoded, &image, entry).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn borrowed_call_free_leaf_requires_retained_replay() {
    let source = std::sync::Arc::new(object_file::stage_optimized_relocation_free_object_container(
        native_text_for("data Record [copy] { value: u16; } machine Record::replace(&write self) { self.value = 17; }", NativeTarget::macos_arm64(), "Record::replace")
    ).unwrap());
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    assert!(
        object
            .functions()
            .iter()
            .all(|function| function.internal_unit_calls.is_empty())
    );
    let mut changed = object.clone();
    changed.clear_fragment_replay_for_test();
    image_emission::validate_function_fragment_object_artifact(&source, &changed).unwrap();
    assert!(image_emission::emit_executable_image(&changed, 3).is_err());
}
