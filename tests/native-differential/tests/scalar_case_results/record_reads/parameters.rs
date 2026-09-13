//! Owned entry values keep their ABI and an exact activation-local readable home.
use super::super::{produce_source, publish};
use target::NativeTarget;
use terminal_codec::CanonicalTerminalArtifact;

fn execute(artifact: &CanonicalTerminalArtifact, driver: &str, windows_direct: bool) {
    let mut targets = vec![
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ];
    if windows_direct {
        targets.push(NativeTarget::windows_x64());
    }
    for target in targets {
        let (image, offset) = publish(artifact, target);
        indirect_input_custody(&image);
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        if target == NativeTarget::host() {
            super::super::native_function::assert_c_text(
                &image.output().final_text_bytes,
                offset,
                driver,
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
            let _ = (image, offset, driver);
            eprintln!(
                "SKIP: owned record entry runtime requires Linux x64/ARM64 or macOS ARM64; cross-target publication was checked"
            );
        }
    }
}

fn indirect_input_custody(image: &image_emission::ExecutableImage) {
    let record = image_emission::build_installation_record(
        image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    for (function_index, function) in record.functions().iter().enumerate() {
        if !function
            .unit_parameter_homes
            .iter()
            .chain(&function.scalar_structural_parameter_homes)
            .any(|home| home.access == terminal_psi::StructuralAccess::Owned && home.indirect)
        {
            continue;
        }
        for mutation in 0..4 {
            let mut changed = record.clone();
            let function = &mut changed.functions_mut_for_test()[function_index];
            let homes = if function.mixed_structural_scalar_abi.is_some() {
                &mut function.scalar_structural_parameter_homes
            } else {
                &mut function.unit_parameter_homes
            };
            let home = homes
                .iter_mut()
                .find(|home| home.access == terminal_psi::StructuralAccess::Owned && home.indirect)
                .unwrap();
            let place = home.place;
            match mutation {
                0 => {
                    home.location = machine_code::StructuralSourceLocation::Stack { byte_offset: 0 }
                }
                1 => home.indirect = false,
                2 => home.place = semantic_vocabulary::PlaceId::new(999).unwrap(),
                _ => {
                    // ABI geometry cannot distinguish affine from linear
                    // custody. Change every projected copy consistently; the
                    // join to the independently checked image must reject it.
                    home.multiplicity = terminal_psi::StructuralMultiplicity::Linear;
                    for parameter in function
                        .unit_parameters
                        .iter_mut()
                        .chain(&mut function.scalar_structural_parameters)
                    {
                        if parameter.place == place {
                            parameter.multiplicity = terminal_psi::StructuralMultiplicity::Linear;
                        }
                    }
                    if let Some(abi) = &mut function.mixed_structural_scalar_abi {
                        for parameter in &mut abi.structural_parameters {
                            if parameter.place == place {
                                parameter.multiplicity =
                                    terminal_psi::StructuralMultiplicity::Linear;
                            }
                        }
                    }
                }
            }
            assert!(
                image_emission::validate_installation_record(&changed, image).is_err(),
                "indirect input mutation {mutation}"
            );
            if let Ok(bytes) = image_emission::encode_installation_record(&changed) {
                let decoded = image_emission::decode_installation_record(&bytes).unwrap();
                assert!(
                    image_emission::validate_installation_record(&decoded, image).is_err(),
                    "reloaded indirect input mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn owned_record_entry_fields_survive_surrounding_calls() {
    for property in ["", "[copy]"] {
        let artifact = produce_source(
            "observe",
            &format!(
                "data Record {property} {{ prefix: u8; payload: u64; }}
             machine identity(value: u64) -> u64 {{ value }}
             machine observe(record: Record, mask: u64) -> u64 {{
                 let before: u64 = identity(record.payload);
                 identity(mask) ^ (record.payload & before)
             }}"
            ),
        );
        // Native signatures retain scalar parameters before structural parameters.
        // The C caller passes actual value bytes, never a pointer to Record.
        execute(
            &artifact,
            r#"
            #include <stdint.h>
            typedef struct { uint8_t prefix; uint64_t payload; } Record;
            extern uint64_t omega_entry(uint64_t mask, Record record);
            int main(void) {
                uint64_t payloads[] = { 0, UINT64_MAX, UINT64_C(0x123456789abcdef0) };
                for (unsigned ordinal = 0; ordinal < 3; ++ordinal) {
                    uint64_t payload = payloads[ordinal];
                    uint64_t mask = UINT64_C(0xfedcba9876543210);
                    Record record = { 17, payload };
                    if (omega_entry(mask, record) != (payload ^ mask)) return 1;
                }
                return 0;
            }
        "#,
            false,
        );
    }
}

#[test]
fn owned_record_entry_signed_field_preserves_its_nonzero_byte_offset() {
    let artifact = produce_source(
        "observe",
        "
        data Record { prefix: u8; selected: bool; payload: i16; }
        machine identity(value: i16) -> i16 { value }
        machine observe(record: Record) -> i16 { identity(record.payload) }",
    );
    execute(
        &artifact,
        r#"
        #include <stdbool.h>
        #include <stdint.h>
        typedef struct { uint8_t prefix; bool selected; int16_t payload; } Record;
        extern int16_t omega_entry(Record record);
        int main(void) {
            int16_t payloads[] = { INT16_MIN, -12345, 0, INT16_MAX };
            for (unsigned ordinal = 0; ordinal < 4; ++ordinal) {
                Record record = { 239, true, payloads[ordinal] };
                if (omega_entry(record) != payloads[ordinal]) return 1;
            }
            return 0;
        }
    "#,
        true,
    );
}

#[test]
fn owned_record_entry_boolean_field_ignores_adjacent_bits() {
    let artifact = produce_source(
        "observe",
        "
        data Record { prefix: u8; selected: bool; payload: i16; }
        machine identity(value: bool) -> bool { value }
        machine observe(record: Record, other: bool) -> bool {
            identity(other) && record.selected
        }",
    );
    execute(
        &artifact,
        r#"
        #include <stdbool.h>
        #include <stdint.h>
        typedef struct { uint8_t prefix; bool selected; int16_t payload; } Record;
        extern bool omega_entry(bool other, Record record);
        int main(void) {
            for (unsigned selected = 0; selected < 2; ++selected) {
                for (unsigned other = 0; other < 2; ++other) {
                    Record record = { 239, selected != 0, -12345 };
                    if (omega_entry(other != 0, record) != (selected && other)) return 1;
                }
            }
            return 0;
        }
    "#,
        true,
    );
}

#[test]
fn owned_record_entry_stack_fragments_survive_calls() {
    let artifact = produce_source(
        "observe",
        "
        data Record { prefix: u8; payload: u64; }
        machine identity(value: u64) -> u64 { value }
        machine observe(first: u64, second: u64, third: u64, fourth: u64,
                        fifth: u64, sixth: u64, seventh: u64, eighth: u64,
                        record: Record) -> u64 {
            identity(first) ^ record.payload
        }",
    );
    execute(
        &artifact,
        r#"
        #include <stdint.h>
        typedef struct { uint8_t prefix; uint64_t payload; } Record;
        extern uint64_t omega_entry(uint64_t first, uint64_t second, uint64_t third,
            uint64_t fourth, uint64_t fifth, uint64_t sixth, uint64_t seventh,
            uint64_t eighth, Record record);
        int main(void) {
            Record record = { 239, UINT64_MAX };
            uint64_t mask = UINT64_C(0x123456789abcdef0);
            return omega_entry(mask, 2, 3, 4, 5, 6, 7, 8, record) == (mask ^ record.payload) ? 0 : 1;
        }
    "#,
        false,
    );
}

#[test]
fn owned_record_entry_observation_and_nested_copy_share_the_value_home() {
    let artifact = produce_source(
        "observe",
        "
        data Record { payload: u64; }
        data Outer { observed: u64; child: Record; }
        machine observe(record: Record) -> Outer {
            Outer { observed: record.payload, child: record }
        }",
    );
    execute(
        &artifact,
        r#"
        #include <stdint.h>
        typedef struct { uint64_t payload; } Record;
        typedef struct { uint64_t observed; Record child; } Outer;
        extern Outer omega_entry(Record record);
        int main(void) {
            Record record = { UINT64_MAX };
            Outer result = omega_entry(record);
            return result.observed == UINT64_MAX && result.child.payload == UINT64_MAX ? 0 : 1;
        }
    "#,
        true,
    );
}

#[test]
fn owned_record_indirect_entry_fields_survive_calls() {
    for stack_pointer in [false, true] {
        let extra_parameters = if stack_pointer {
            ", second: u64, third: u64, fourth: u64, fifth: u64, sixth: u64, seventh: u64, eighth: u64"
        } else {
            ""
        };
        let artifact = produce_source(
            "observe",
            &format!(
                "data Record {{ prefix: u64; middle: u64; payload: u64; }}
                 machine identity(value: u64) -> u64 {{ value }}
                 machine observe(mask: u64{extra_parameters}, record: Record) -> u64 {{
                     let prefix_value: u64 = identity(record.prefix);
                     let middle_value: u64 = identity(record.middle);
                     identity(mask) ^ prefix_value ^ middle_value ^ record.payload
                 }}"
            ),
        );
        let signature = if stack_pointer {
            "uint64_t mask, uint64_t second, uint64_t third, uint64_t fourth, uint64_t fifth, uint64_t sixth, uint64_t seventh, uint64_t eighth, Record record"
        } else {
            "uint64_t mask, Record record"
        };
        let arguments = if stack_pointer {
            "mask, 2, 3, 4, 5, 6, 7, 8, record"
        } else {
            "mask, record"
        };
        // A large value is indirect on ARM64 and Windows, inline on SysV x64.
        // Eight preceding scalar arguments also put the indirect pointer on
        // the incoming stack; those pointer bits are not the record's payload.
        execute(
            &artifact,
            &format!(
                "#include <stdint.h>
                 typedef struct {{ uint64_t prefix; uint64_t middle; uint64_t payload; }} Record;
                 extern uint64_t omega_entry({signature});
                 int main(void) {{
                     uint64_t payloads[] = {{ 0, UINT64_MAX, UINT64_C(0x123456789abcdef0) }};
                     for (unsigned ordinal = 0; ordinal < 3; ++ordinal) {{
                         uint64_t mask = UINT64_C(0xfedcba9876543210);
                         Record record = {{ UINT64_C(0xc001d00d), UINT64_C(0x8000000000000001), payloads[ordinal] }};
                         uint64_t expected = mask ^ record.prefix ^ record.middle ^ record.payload;
                         if (omega_entry({arguments}) != expected) return 1;
                     }}
                     return 0;
                 }}"
            ),
            true,
        );
    }
}

#[test]
fn owned_record_indirect_entry_can_move_into_a_nested_result() {
    let artifact = produce_source(
        "wrap",
        "data Record { first: u64; second: u64; third: u64; }
         data Outer { child: Record; }
         machine wrap(record: Record) -> Outer { Outer { child: record } }",
    );
    execute(
        &artifact,
        r#"
        #include <stdint.h>
        typedef struct { uint64_t first; uint64_t second; uint64_t third; } Record;
        typedef struct { Record child; } Outer;
        extern Outer omega_entry(Record record);
        int main(void) {
            Record record = { UINT64_MAX, UINT64_C(0x123456789abcdef0), UINT64_C(0x8000000000000001) };
            Outer result = omega_entry(record);
            return result.child.first == record.first && result.child.second == record.second
                && result.child.third == record.third ? 0 : 1;
        }
        "#,
        true,
    );
}

#[test]
fn owned_record_parameter_returns_through_indirect_result_storage() {
    for property in ["", "[copy]"] {
        for stack_pointer in [false, true] {
            let extra_parameters = if stack_pointer {
                ", second: u64, third: u64, fourth: u64, fifth: u64, sixth: u64, seventh: u64, eighth: u64"
            } else {
                ""
            };
            let artifact = produce_source(
                "retain",
                &format!(
                    "data Record {property} {{ first: u64; second: u64; third: u64; }}
                     machine identity(value: u64) -> u64 {{ value }}
                     machine retain(mask: u64{extra_parameters}, record: Record) -> Record {{
                         _ = identity(mask);
                         record
                     }}"
                ),
            );
            let signature = if stack_pointer {
                "uint64_t mask, uint64_t second, uint64_t third, uint64_t fourth, uint64_t fifth, uint64_t sixth, uint64_t seventh, uint64_t eighth, Record record"
            } else {
                "uint64_t mask, Record record"
            };
            let arguments = if stack_pointer {
                "UINT64_MAX, 2, 3, 4, 5, 6, 7, 8, record"
            } else {
                "UINT64_MAX, record"
            };
            execute(
                &artifact,
                &format!(
                    "#include <stdint.h>
                     typedef struct {{ uint64_t first; uint64_t second; uint64_t third; }} Record;
                     extern Record omega_entry({signature});
                     int main(void) {{
                         Record record = {{ UINT64_MAX, UINT64_C(0x123456789abcdef0), UINT64_C(0x8000000000000001) }};
                         Record result = omega_entry({arguments});
                         return result.first == record.first && result.second == record.second
                             && result.third == record.third ? 0 : 1;
                     }}"
                ),
                true,
            );
        }
    }
}

#[test]
fn owned_record_parameter_round_trip_uses_independent_input_and_result_abi() {
    let artifact = produce_source(
        "retain",
        "data Record { first: u64; second: u64; third: u64; }
         machine retain(record: Record) -> Record { record }",
    );
    execute(
        &artifact,
        r#"
        #include <stdint.h>
        typedef struct { uint64_t first; uint64_t second; uint64_t third; } Record;
        extern Record omega_entry(Record record);
        int main(void) {
            Record record = { UINT64_MAX, UINT64_C(0x123456789abcdef0), UINT64_C(0x8000000000000001) };
            Record result = omega_entry(record);
            return result.first == record.first && result.second == record.second
                && result.third == record.third ? 0 : 1;
        }
        "#,
        true,
    );
}

#[test]
fn owned_record_parameter_return_survives_an_observable_call() {
    let artifact = produce_source(
        "retain",
        "data Record { first: u64; second: u64; third: u64; }
         machine replace(value: &mut u64, replacement: u64) -> u64 {
             value = replacement;
             replacement
         }
         machine retain(record: Record, output: &mut u64, replacement: u64) -> Record {
             _ = replace(&mut output, replacement);
             record
         }",
    );
    execute(
        &artifact,
        r#"
        #include <stdint.h>
        typedef struct { uint64_t first; uint64_t second; uint64_t third; } Record;
        extern Record omega_entry(uint64_t replacement, Record record, uint64_t *output);
        int main(void) {
            Record record = { UINT64_MAX, UINT64_C(0x123456789abcdef0), UINT64_C(0x8000000000000001) };
            uint64_t output = 0;
            Record result = omega_entry(UINT64_C(0xfedcba9876543210), record, &output);
            return output == UINT64_C(0xfedcba9876543210) && result.first == record.first
                && result.second == record.second && result.third == record.third ? 0 : 1;
        }
        "#,
        true,
    );
}
