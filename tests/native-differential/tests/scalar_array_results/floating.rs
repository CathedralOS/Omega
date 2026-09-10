//! Floating leaves retain IEEE bits inside the existing integer-fragment array ABI.
use super::*;

fn source(carrier: &str, shape: &str, initializer: &str) -> String {
    format!(
        "machine keep(row: {shape}) -> {shape} {{ row }}
         machine forward(row: {shape}) -> {shape} {{ keep(row) }}
         machine make(value: {carrier}) -> {shape} {{ {initializer} }}
         machine selected(value: {carrier}) -> {shape} {{
             let row: {shape} = make(value);
             let copied: {shape} = keep(row);
             forward(copied)
         }}"
    )
}

#[test]
fn floating_parameters_and_owned_calls_preserve_bits() {
    for (carrier, shape, initializer, c_type, bits_type, patterns, expected) in [
        (
            "f32",
            "[f32; 2]",
            "[value, -0.0f32]",
            "float",
            "uint32_t",
            "{0, 0x80000000U, 1, 0x7f800000U, 0xff800000U, 0x7fc01234U, 0x3fc00000U}",
            "((uint64_t)bits | 0x8000000000000000ULL)",
        ),
        (
            "f64",
            "[f64; 1]",
            "[value]",
            "double",
            "uint64_t",
            "{0, 0x8000000000000000ULL, 1, 0x7ff0000000000000ULL, 0xfff0000000000000ULL, 0x7ff8000000001234ULL, 0x3ff8000000000000ULL}",
            "bits",
        ),
        (
            "f32",
            "[[f32; 2]; 1]",
            "[[value, -0.0f32]]",
            "float",
            "uint32_t",
            "{0, 0x80000000U, 1, 0x7f800000U, 0xff800000U, 0x7fc01234U, 0x3fc00000U}",
            "((uint64_t)bits | 0x8000000000000000ULL)",
        ),
    ] {
        let source = source(carrier, shape, initializer);
        for entry in ["make", "forward"] {
            for target in [
                NativeTarget::linux_x64(),
                NativeTarget::linux_arm64(),
                NativeTarget::macos_arm64(),
                NativeTarget::windows_x64(),
            ] {
                let (image, offset) = publish(&source, entry, target);
                #[cfg(any(
                    all(
                        target_os = "linux",
                        any(target_arch = "x86_64", target_arch = "aarch64")
                    ),
                    all(target_os = "macos", target_arch = "aarch64")
                ))]
                if target == NativeTarget::host() {
                    // Omega arrays currently use integer aggregate fragments, not
                    // the foreign C homogeneous-floating-aggregate convention.
                    // Observe those exact bytes without performing float arithmetic.
                    let (argument_type, argument) = if entry == "make" {
                        (c_type, "value")
                    } else {
                        ("uint64_t", "expected")
                    };
                    let driver = format!(
                    "#include <stdint.h>\n#include <string.h>\n
                     extern uint64_t omega_entry({argument_type});
                     int main(void) {{
                         const {bits_type} patterns[] = {patterns};
                         for (unsigned pattern = 0; pattern < sizeof(patterns) / sizeof(patterns[0]); ++pattern) {{
                             {bits_type} bits = patterns[pattern];
                             {c_type} value; memcpy(&value, &bits, sizeof(value));
                             uint64_t expected = {expected};
                             if (omega_entry({argument}) != expected) return 1;
                         }}
                         return 0;
                     }}"
                );
                    native_function::assert_c_text(
                        &image.output().final_text_bytes,
                        offset,
                        &driver,
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
                    let _ = (image, offset, c_type, bits_type, patterns, expected);
                    eprintln!("SKIP: floating array runtime requires the Linux/macOS host harness");
                }
            }
        }
    }
}

#[test]
fn ieee_scalar_arguments_with_array_results_require_mixed_call_constraints() {
    for (carrier, shape, initializer) in [
        ("f32", "[f32; 2]", "[value, -0.0f32]"),
        ("f64", "[f64; 1]", "[value]"),
    ] {
        // Preserve the full caller: direct construction and owned forwarding
        // work, but calling make still needs a mixed-bank aggregate-call row.
        let source = source(carrier, shape, initializer);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let plan = target_plan(&source, "selected", target).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let result =
                target_operations_to_selected_instructions::stage_optimized_instruction_selection(
                    plan,
                    environment,
                );
            assert!(matches!(result, Err(
                target_operations_to_selected_instructions::OptimizedSelectionPipelineError::Selection(
                    target_operations_to_selected_instructions::SelectedInstructionError::SourceCustodyMismatch
                )
            )), "mixed scalar/aggregate call needs its exact constraint row on {target:?}: {result:?}");
        }
    }
}
