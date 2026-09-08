//! Literal definitions keep their exact IEEE format through borrowed stores and calls.
use super::*;

#[test]
fn ieee_field_literals_preserve_projected_caller_bytes() {
    literal_stores(false);
}

#[test]
fn ieee_call_literals_preserve_projected_caller_bytes() {
    literal_stores(true);
}

#[test]
fn ieee_call_literals_cross_float_register_and_stack_arguments() {
    let mut fields = String::new();
    let mut parameters = String::new();
    let mut assignments = String::new();
    let mut arguments = String::new();
    let mut native_fields = String::new();
    let mut native_expected = String::new();
    for field_index in 0..9 {
        let is_binary32 = field_index % 2 == 0;
        let (scalar, native_scalar, native_bits) = if is_binary32 {
            ("f32", "float", "uint32_t")
        } else {
            ("f64", "double", "uint64_t")
        };
        let negative_zero = field_index % 3 == 1;
        let literal = if negative_zero { "-0.0" } else { "1.25" };
        let bits = match (is_binary32, negative_zero) {
            (true, true) => 0x8000_0000_u64,
            (false, true) => 0x8000_0000_0000_0000_u64,
            (true, false) => u64::from(1.25_f32.to_bits()),
            (false, false) => 1.25_f64.to_bits(),
        };
        fields.push_str(&format!("field_{field_index}: {scalar};"));
        parameters.push_str(&format!(", value_{field_index}: {scalar}"));
        assignments.push_str(&format!("self.field_{field_index} = value_{field_index};"));
        if field_index != 0 {
            arguments.push_str(", ");
        }
        arguments.push_str(literal);
        native_fields.push_str(&format!("{native_scalar} field_{field_index};"));
        native_expected.push_str(&format!(
            "{native_bits} bits_{field_index} = {bits}ull;
            memcpy(&expected.record.field_{field_index}, &bits_{field_index}, sizeof bits_{field_index});"
        ));
    }
    let source = format!(
        "data Record [copy] {{ before: u8; {fields} after: u8; }}
        machine Record::replace(&write self{parameters}) {{ {assignments} }}
        machine forward(record: &write Record) {{
            record.replace({arguments});
            record.replace({arguments});
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
        ieee_stores::assert_ieee_c_text(
            &bytes,
            entry,
            &format!(
                r#"
            #include <stdint.h>
            #include <string.h>
            typedef struct {{ uint8_t before; {native_fields} uint8_t after; }} Record;
            extern void omega_entry(Record *record);
            int main(void) {{
                struct {{ uint64_t before; Record record; uint64_t after; }} frame, expected;
                memset(&frame, 0xa5, sizeof frame);
                memcpy(&expected, &frame, sizeof frame);
                {native_expected}
                omega_entry(&frame.record);
                return memcmp(&expected, &frame, sizeof frame) != 0;
            }}
        "#
            ),
        );
    }
}

fn literal_stores(call_literal: bool) {
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
            for access in ["write", "mut"] {
                let stored = if call_literal { "value" } else { literal };
                let argument = if call_literal { literal } else { "unused" };
                let source = format!(
                    "data Record [copy] {{ before: u8; value: {scalar}; after: u8; }}
                    machine Record::replace(&write self, value: {scalar}) {{ self.value = {stored}; }}
                    machine forward(records: &{access} [Record; 2], unused: {scalar}) {{
                        let held: &write [Record; 2] = &write records;
                        let child: &write Record = &write held[1];
                        child.replace({argument});
                        child.replace({argument});
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
                    ieee_stores::assert_ieee_c_text(
                        &bytes,
                        entry,
                        &format!(
                            r#"
                        #include <stdint.h>
                        #include <string.h>
                        typedef struct {{ uint8_t before; {native_scalar} value; uint8_t after; }} Record;
                        extern void omega_entry({native_scalar} unused, Record *records);
                        int main(void) {{
                            struct {{ uint64_t before; Record records[2]; uint64_t after; }} frame;
                            memset(&frame, 0xa5, sizeof frame);
                            {native_bits} bits = {expected_bits}ull;
                            memcpy(&frame.records[1].value, &bits, sizeof bits);
                            unsigned char expected[sizeof frame];
                            memcpy(expected, &frame, sizeof frame);
                            memset(&frame.records[1].value, 0x5a, sizeof frame.records[1].value);
                            omega_entry(19.5, frame.records);
                            return memcmp(expected, &frame, sizeof frame) != 0;
                        }}
                    "#
                        ),
                    );
                }
            }
        }
    }
}
