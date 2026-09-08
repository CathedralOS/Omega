//! Whole-scalar replacement uses its original primitive referent through calls.
use super::*;

const FORWARDED_SCALAR: &str = "machine replace(destination: &write i32, value: i32) {
    destination = value;
}
machine forward(destination: &write i32, value: i32) {
    replace(&write destination, value);
}";

#[test]
fn forwarded_primitive_store_publishes_on_hosted_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let _ = published_text(FORWARDED_SCALAR, target);
    }
}

pub(super) fn published_text(source_text: &str, target: NativeTarget) -> (Vec<u8>, usize) {
    let placed = native_text(source_text, target);
    let entry = placed.text_section().semantic_entry;
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(placed).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    let entry_offset = object
        .functions()
        .iter()
        .find(|function| function.machine == entry)
        .unwrap()
        .text_offset;
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    let installed = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let bytes = image_emission::encode_installation_record(&installed).unwrap();
    let decoded = image_emission::decode_installation_record(&bytes).unwrap();
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    (image.output().final_text_bytes.clone(), entry_offset)
}

#[test]
fn boolean_primitive_installation_rejects_borrowed_contract_substitution() {
    let placed = native_text(
        &FORWARDED_SCALAR.replace("i32", "bool"),
        NativeTarget::macos_arm64(),
    );
    let entry = placed.text_section().semantic_entry;
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(placed).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source).unwrap();
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    image_emission::validate_installation_record(&record, &image).unwrap();
    for mutation in 0..4 {
        let mut changed = record.clone();
        let caller = changed
            .functions_mut_for_test()
            .iter_mut()
            .find(|function| function.machine == entry)
            .unwrap();
        match mutation {
            0 => caller.unit_parameter_homes.clear(),
            1 => caller.unit_parameter_homes[0].source.shape.byte_size += 1,
            2 => {
                caller.unit_parameter_homes[0].access = terminal_psi::StructuralAccess::SharedBorrow
            }
            3 => {
                caller.unit_scalar_abi.as_mut().unwrap().parameters[0].scalar_type =
                    semantic_vocabulary::ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            8,
                        )
                        .unwrap(),
                    )
            }
            _ => unreachable!(),
        }
        if mutation < 3 {
            assert!(
                image_emission::encode_installation_record(&changed).is_err(),
                "mutation {mutation}"
            );
        }
        assert!(
            image_emission::validate_installation_record(&changed, &image).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn primitive_stack_pointer_keeps_original_referent_across_three_calls() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let prefix = if target == NativeTarget::windows_x64() {
            4
        } else if target == NativeTarget::linux_x64() {
            6
        } else {
            8
        };
        let parameters = (0..prefix)
            .map(|position| format!(", argument{position}: u64"))
            .collect::<String>();
        let arguments = (0..prefix)
            .map(|position| format!(", argument{position}"))
            .collect::<String>();
        let call = format!("replace(&write destination{arguments});");
        let source = format!(
            "machine replace(destination: &write u64{parameters}) {{
                destination = argument{};
            }}
            machine forward(destination: &write u64{parameters}) {{ {call} {call} {call} }}",
            prefix - 1
        );
        let (bytes, entry_offset) = published_text(&source, target);
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
        {
            let parameters = "uint64_t, ".repeat(prefix);
            let seed = 0xfedc_ba98_7654_3200_u64;
            let arguments = (0..prefix)
                .map(|position| format!("{}ull, ", seed + position as u64))
                .collect::<String>();
            let expected = seed + prefix as u64 - 1;
            native_function::assert_c_text(
                &bytes,
                entry_offset,
                &format!(
                    r#"
                #include <stdint.h>
                #include <string.h>
                extern void omega_entry({parameters}uint64_t *destination);
                int main(void) {{
                    struct {{ uint64_t before; uint64_t value; uint64_t after; }} frame;
                    memset(&frame, 0xa5, sizeof frame);
                    frame.value = {expected}ull;
                    unsigned char expected[sizeof frame];
                    memcpy(expected, &frame, sizeof frame);
                    frame.value = 0;
                    omega_entry({arguments}&frame.value);
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
            let _ = (bytes, entry_offset);
            eprintln!(
                "SKIP: primitive stack-pointer execution requires a supported Linux or macOS host"
            );
        }
    }
}

#[test]
fn primitive_stores_preserve_exact_runtime_width_and_neighbor_bytes() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    for (scalar, native_scalar, runtime_value, literal) in [
        ("i8", "int8_t", "-128", "-17"),
        ("i16", "int16_t", "-32768", "-17"),
        ("i32", "int32_t", "-2147483647-1", "-17"),
        ("i64", "int64_t", "-9223372036854775807ll-1", "-17"),
        ("u8", "uint8_t", "255", "17"),
        ("u16", "uint16_t", "65535", "17"),
        ("u32", "uint32_t", "4294967295u", "17"),
        ("u64", "uint64_t", "18364758544493064720ull", "17"),
        ("bool", "_Bool", "1", "true"),
        ("bool", "_Bool", "0", "false"),
    ] {
        for access in ["write", "mut"] {
            for immediate in [false, true] {
                let assigned = if immediate { literal } else { "value" };
                eprintln!(
                    "primitive store: {scalar}, {access}, immediate={immediate}, value={assigned}"
                );
                let source = format!("machine replace(destination: &{access} {scalar}, value: {scalar}) {{ destination = {assigned}; }}
                    machine forward(destination: &{access} {scalar}, value: {scalar}) {{
                        replace(&{access} destination, value);
                        replace(&{access} destination, value);
                    }}");
                let (bytes, entry_offset) = published_text(&source, NativeTarget::host());
                let expected = if immediate {
                    match literal {
                        "true" => "1",
                        "false" => "0",
                        _ => literal,
                    }
                } else {
                    runtime_value
                };
                native_function::assert_c_text(
                    &bytes,
                    entry_offset,
                    &format!(
                        r#"
                    #include <stdint.h>
                    #include <string.h>
                    extern void omega_entry({native_scalar} value, {native_scalar} *destination);
                    int main(void) {{
                        struct {{ uint64_t before; {native_scalar} value; uint64_t after; }} frame;
                        memset(&frame, 0xa5, sizeof frame);
                        frame.value = ({native_scalar})({expected});
                        unsigned char expected[sizeof frame];
                        memcpy(expected, &frame, sizeof frame);
                        frame.value = ({native_scalar})(!({expected}));
                        omega_entry(({native_scalar})({runtime_value}), &frame.value);
                        return memcmp(expected, &frame, sizeof frame) != 0;
                    }}
                "#
                    ),
                );
            }
        }
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: primitive caller observation requires a supported Linux or macOS native host");
}
