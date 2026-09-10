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
        for entry in ["make", "forward", "selected"] {
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
                    let (argument_type, argument) = if entry != "forward" {
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
fn interleaved_ieee_and_integer_arguments_preserve_aggregate_results() {
    // The authored float/integer/float order differs from the constraint's
    // GPR-then-IEEE order, including Microsoft's positional register holes.
    let source = "machine make(first: f32, marker: u64, second: f32) -> [f32; 2] {
                      [first, second]
                  }
                  machine selected(first: f32, marker: u64, second: f32) -> [f32; 2] {
                      make(first, marker, second)
                  }";
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
                "#include <stdint.h>\n#include <string.h>\n
                     extern uint64_t omega_entry(float, uint64_t, float);
                     int main(void) {
                         uint32_t first_bits = 0x7fc01234U, second_bits = 0x80000000U;
                         float first, second;
                         memcpy(&first, &first_bits, sizeof(first));
                         memcpy(&second, &second_bits, sizeof(second));
                         return omega_entry(first, 37, second) != 0x800000007fc01234ULL;
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
            eprintln!("SKIP: mixed aggregate call runtime requires the Linux/macOS host harness");
        }
    }
}

#[test]
fn ieee_arguments_compose_with_two_fragment_owned_arguments_and_results() {
    let source = "machine keep(first: f64, row: [u64; 2], last: f64) -> [u64; 2] { row }
                  machine selected(value: u64, first: f64, last: f64) -> [u64; 2] {
                      keep(first, [value, 37], last)
                  }";
    // Microsoft uses the separately unsupported hidden-pointer result ABI here.
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
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
                "#include <stdint.h>\n
                 typedef struct { uint64_t first; uint64_t second; } Pair;
                 extern Pair omega_entry(uint64_t, double, double);
                 int main(void) {
                     Pair row = omega_entry(0xfedcba9876543210ULL, 1.5, -0.0);
                     return row.first != 0xfedcba9876543210ULL || row.second != 37;
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
            eprintln!("SKIP: mixed aggregate call runtime requires the Linux/macOS host harness");
        }
    }
}
