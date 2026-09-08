//! IEEE stores preserve runtime payload bits in the original borrowed storage.
use super::*;

#[test]
fn ieee_field_store_reaches_canonical_terminal() {
    for scalar in ["f32", "f64"] {
        let source = format!(
            "data Record [copy] {{ value: {scalar}; }}
            machine Record::replace(&write self, value: {scalar}) {{
                self.value = value;
            }}
            machine forward(records: &write [Record; 2], value: {scalar}) {{
                records[1].replace(value);
            }}"
        );
        let _ = artifact(&source);
    }
}

#[test]
fn ieee_field_store_encoding_rejects_format_and_access_drift() {
    use semantic_vocabulary::{IeeeFloatFormat, ScalarType};
    for (scalar, wrong_format) in [
        ("f32", IeeeFloatFormat::Binary64),
        ("f64", IeeeFloatFormat::Binary32),
    ] {
        let source = format!(
            "data Record [copy] {{ value: {scalar}; }}
            machine Record::replace(&write self, value: {scalar}) {{ self.value = value; }}
            machine forward(records: &write [Record; 2], value: {scalar}) {{ records[1].replace(value); }}"
        );
        let artifact = artifact(&source);
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        for corruption in 0..3 {
            let mut changed = module.clone();
            let store = changed
                .machines
                .iter_mut()
                .find(|machine| {
                    machine.blocks.iter().any(|block| {
                        block.operations.iter().any(|operation| {
                            matches!(
                                operation.kind,
                                terminal_psi::OperationKind::StructuralScalarFieldStore { .. }
                            )
                        })
                    })
                })
                .unwrap();
            match corruption {
                0 => store.parameters[0].scalar_type = ScalarType::IeeeFloat(wrong_format),
                1 => {
                    store.structural_parameters[0].access =
                        terminal_psi::StructuralAccess::SharedBorrow
                }
                2 => {
                    let field = changed
                        .structural_types
                        .iter_mut()
                        .find_map(|declaration| {
                            let terminal_psi::StructuralTypeShape::Record { fields } =
                                &mut declaration.shape
                            else {
                                return None;
                            };
                            fields.iter_mut().find(|field| {
                                matches!(
                                    field.field_type,
                                    terminal_psi::StructuralFieldType::IeeeFloat(_)
                                )
                            })
                        })
                        .unwrap();
                    field.field_type = terminal_psi::StructuralFieldType::IeeeFloat(wrong_format);
                }
                _ => unreachable!(),
            }
            assert!(
                terminal_codec::encode_module(&changed).is_err(),
                "corruption {corruption}"
            );
            assert!(
                terminal_verifier::validate_module(&changed).is_err(),
                "corruption {corruption}"
            );
        }
    }
}

#[test]
fn ieee_primitive_store_publishes_on_hosted_targets() {
    for scalar in ["f32", "f64"] {
        let source = format!(
            "machine replace(destination: &write {scalar}, value: {scalar}) {{
                destination = value;
            }}
            machine forward(destination: &write {scalar}, value: {scalar}) {{
                replace(&write destination, value);
            }}"
        );
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let _ = primitive_stores::published_text(&source, target);
        }
    }
}

#[test]
fn ieee_stores_preserve_runtime_payloads_and_neighbor_bytes() {
    for (scalar, native_scalar, native_bits, payloads) in [
        (
            "f32",
            "float",
            "uint32_t",
            "0u, 0x80000000u, 1u, 0x007fffffu, 0x00800000u, 0x7f7fffffu, 0x7f800000u, 0xff800000u, 0x7fc12345u, 0xffc54321u, 0x7f812345u",
        ),
        (
            "f64",
            "double",
            "uint64_t",
            "0ull, 0x8000000000000000ull, 1ull, 0x000fffffffffffffull, 0x0010000000000000ull, 0x7fefffffffffffffull, 0x7ff0000000000000ull, 0xfff0000000000000ull, 0x7ff8123456789abcull, 0xfff854321fedcba9ull, 0x7ff0123456789abcull",
        ),
    ] {
        for access in ["write", "mut"] {
            for projected in [false, true] {
                let (source, native_referent, destination, invocation) = if projected {
                    (
                        format!(
                            "data Record [copy] {{ before: u8; value: {scalar}; after: u8; }}
                            machine Record::replace(&write self, value: {scalar}) {{ self.value = value; }}
                            machine forward(records: &{access} [Record; 2], value: {scalar}) {{
                                let held: &write [Record; 2] = &write records;
                                let child: &write Record = &write held[1];
                                child.replace(value);
                                child.replace(value);
                            }}"
                        ),
                        "Record records[2]".to_owned(),
                        "frame.records[1].value",
                        "frame.records",
                    )
                } else {
                    (
                        format!(
                            "machine replace(destination: &write {scalar}, value: {scalar}) {{ destination = value; }}
                            machine forward(destination: &{access} {scalar}, value: {scalar}) {{
                                replace(&write destination, value);
                                replace(&write destination, value);
                            }}"
                        ),
                        format!("{native_scalar} value"),
                        "frame.value",
                        "&frame.value",
                    )
                };
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
                    let native_pointer = if projected { "Record" } else { native_scalar };
                    assert_ieee_c_text(
                        &bytes,
                        entry,
                        &format!(
                            r#"
                        #include <stdint.h>
                        #include <string.h>
                        typedef struct {{ uint8_t before; {native_scalar} value; uint8_t after; }} Record;
                        extern void omega_entry({native_scalar} value, {native_pointer} *destination);
                        int main(void) {{
                            const {native_bits} payloads[] = {{ {payloads} }};
                            for (unsigned occurrence = 0; occurrence < sizeof payloads / sizeof payloads[0]; ++occurrence) {{
                                struct {{ uint64_t before; {native_referent}; uint64_t after; }} frame;
                                memset(&frame, 0xa5, sizeof frame);
                                memcpy(&{destination}, &payloads[occurrence], sizeof({native_bits}));
                                unsigned char expected[sizeof frame];
                                memcpy(expected, &frame, sizeof frame);
                                memset(&{destination}, 0x5a, sizeof({native_bits}));
                                {native_scalar} value;
                                memcpy(&value, &payloads[occurrence], sizeof value);
                                omega_entry(value, {invocation});
                                if (memcmp(expected, &frame, sizeof frame) != 0) return occurrence + 1;
                            }}
                            return 0;
                        }}
                        "#
                        ),
                    );
                }
            }
        }
    }
}

#[test]
fn mixed_ieee_and_integer_arguments_preserve_register_and_stack_payloads() {
    for count in [2, 9] {
        let mut fields = String::new();
        let mut parameters = String::new();
        let mut arguments = String::new();
        let mut stores = String::new();
        let mut native_fields = String::new();
        let mut native_parameters = String::new();
        let mut native_arguments = String::new();
        let mut native_setup = String::new();
        let mut native_clear = String::new();
        for position in 0..count {
            let (scalar, native_scalar, native_bits, payload) = if position % 2 == 0 {
                ("f32", "float", "uint32_t", 0x7fc1_2345_u64 + position)
            } else {
                (
                    "f64",
                    "double",
                    "uint64_t",
                    0xfff8_1234_5678_9000_u64 + position,
                )
            };
            fields.push_str(&format!("tag{position}: u64; value{position}: {scalar};\n"));
            parameters.push_str(&format!(", tag{position}: u64, value{position}: {scalar}"));
            arguments.push_str(&format!("tag{position}, value{position}, "));
            stores.push_str(&format!(
                "self.tag{position} = tag{position}; self.value{position} = value{position};\n"
            ));
            native_fields.push_str(&format!(
                "uint64_t tag{position}; {native_scalar} value{position};\n"
            ));
            native_parameters.push_str(&format!(
                "uint64_t tag{position}, {native_scalar} value{position}, "
            ));
            native_arguments.push_str(&format!("tag{position}, value{position}, "));
            native_clear.push_str(&format!(
                "frame.record.tag{position} = 0; memset(&frame.record.value{position}, 0, sizeof frame.record.value{position});\n"
            ));
            native_setup.push_str(&format!(
                "uint64_t tag{position} = {}ull;
                {native_bits} bits{position} = {payload}ull;
                {native_scalar} value{position};
                memcpy(&value{position}, &bits{position}, sizeof value{position});
                frame.record.tag{position} = tag{position};
                memcpy(&frame.record.value{position}, &bits{position}, sizeof value{position});\n",
                0xfedc_ba98_7654_3200_u64 + position
            ));
        }
        let arguments = arguments.trim_end_matches(", ");
        let call = format!("record.replace({arguments});");
        let source = format!(
            "data Record [copy] {{ {fields} }}
            machine Record::replace(&write self{parameters}) {{ {stores} }}
            machine forward(record: &write Record{parameters}) {{ {call} {call} {call} }}"
        );
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
            assert_ieee_c_text(
                &bytes,
                entry,
                &format!(
                    r#"
                #include <stdint.h>
                #include <string.h>
                typedef struct {{ {native_fields} }} Record;
                extern void omega_entry({native_parameters}Record *record);
                int main(void) {{
                    struct {{ uint64_t before; Record record; uint64_t after; }} frame;
                    memset(&frame, 0xa5, sizeof frame);
                    {native_setup}
                    unsigned char expected[sizeof frame];
                    memcpy(expected, &frame, sizeof frame);
                    {native_clear}
                    omega_entry({native_arguments}&frame.record);
                    return memcmp(expected, &frame, sizeof frame) != 0;
                }}
                "#
                ),
            );
        }
    }
}

#[test]
fn ieee_primitive_literals_preserve_exact_bits_with_unused_runtime_inputs() {
    for (scalar, native_scalar, native_bits, positive_bits, negative_zero_bits) in [
        (
            "f32",
            "float",
            "uint32_t",
            u64::from(1.25_f32.to_bits()),
            0x8000_0000_u64,
        ),
        (
            "f64",
            "double",
            "uint64_t",
            1.25_f64.to_bits(),
            0x8000_0000_0000_0000_u64,
        ),
    ] {
        for (literal, expected_bits) in [("1.25", positive_bits), ("-0.0", negative_zero_bits)] {
            let source = format!(
                "machine replace(destination: &write {scalar}, unused: {scalar}) {{ destination = {literal}; }}
                machine forward(destination: &write {scalar}, unused: {scalar}) {{
                    replace(&write destination, unused);
                    replace(&write destination, unused);
                }}"
            );
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
                assert_ieee_c_text(
                    &bytes,
                    entry,
                    &format!(
                        r#"
                    #include <stdint.h>
                    #include <string.h>
                    extern void omega_entry({native_scalar} unused, {native_scalar} *destination);
                    int main(void) {{
                        struct {{ uint64_t before; {native_scalar} value; uint64_t after; }} frame;
                        memset(&frame, 0xa5, sizeof frame);
                        {native_bits} bits = {expected_bits}ull;
                        memcpy(&frame.value, &bits, sizeof bits);
                        unsigned char expected[sizeof frame];
                        memcpy(expected, &frame, sizeof frame);
                        memset(&frame.value, 0x5a, sizeof frame.value);
                        omega_entry(19.5, &frame.value);
                        return memcmp(expected, &frame, sizeof frame) != 0;
                    }}
                    "#
                    ),
                );
            }
        }
    }
}

pub(super) fn assert_ieee_c_text(bytes: &[u8], entry: usize, source: &str) {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    native_function::assert_c_text(bytes, entry, source);
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        let _ = (bytes, entry, source);
        eprintln!("SKIP: IEEE caller observation requires a supported Linux or macOS native host");
    }
}
