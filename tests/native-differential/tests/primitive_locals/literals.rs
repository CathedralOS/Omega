//! Literal definitions retain their types when establishing and replacing locals.

use super::{produce, publication};

#[test]
fn boolean_and_ieee_literals_initialize_and_replace_local_storage() {
    for (scalar, carrier, initial, replacement, initial_bits, replacement_bits) in [
        ("bool", "_Bool", "false", "true", "0", "1"),
        ("f32", "float", "-0.0", "1.25", "0x80000000", "0x3fa00000"),
        (
            "f64",
            "double",
            "-0.0",
            "1.25",
            "0x8000000000000000ULL",
            "0x3ff4000000000000ULL",
        ),
    ] {
        let source = format!(
            "machine replace(destination: &mut {scalar}, value: {scalar}) {{
                destination = value;
            }}
            machine observe(output: &mut {scalar}, initial_output: &mut {scalar}) {{
                let mut scratch: {scalar} = {initial};
                initial_output = scratch;
                replace(&mut scratch, {initial});
                scratch = {replacement};
                output = scratch;
            }}"
        );
        let artifact = produce(&source, "observe");
        publication::assert_four_targets(&artifact);
        let driver = format!(
            "#include <stdint.h>
            #include <string.h>
            extern void omega_entry({carrier} *output, {carrier} *initial_output);
            int main(void) {{
                {carrier} output, initial_output;
                memset(&output, 0x5a, sizeof(output));
                memset(&initial_output, 0x5a, sizeof(initial_output));
                const uint64_t expected_initial = {initial_bits};
                const uint64_t expected_output = {replacement_bits};
                memcpy(&output, &expected_initial, sizeof(output));
                memcpy(&initial_output, &expected_output, sizeof(initial_output));
                omega_entry(&output, &initial_output);
                return memcmp(&output, &expected_output, sizeof(output)) != 0
                    || memcmp(&initial_output, &expected_initial, sizeof(initial_output)) != 0;
            }}"
        );
        publication::assert_host_execution(&artifact, &driver);
    }
}
