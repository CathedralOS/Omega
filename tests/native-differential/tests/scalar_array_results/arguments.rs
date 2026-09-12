//! The complete module customer must transport owned payloads, not just construct them.
use super::*;
use compiler::CheckedCompileRequest;

#[test]
fn array_local_control_preserves_selected_returns_and_prefix_effects() {
    let source = include_str!("../../../omega/pass/collections/array_local_control/main.omg");
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
                r"
                #include <stdint.h>
                #include <stdbool.h>
                typedef struct { uint8_t values[2]; } Row;
                extern Row omega_entry(bool, uint8_t);
                int main(void) {
                    for (unsigned value = 0; value < 256; ++value) {
                        Row taken = omega_entry(true, (uint8_t)value);
                        Row fallback = omega_entry(false, (uint8_t)value);
                        if (taken.values[0] != value || taken.values[1] != 9) return 1;
                        if (fallback.values[0] != 13 || fallback.values[1] != 9) return 2;
                    }
                    return 0;
                }
            ",
            );
        } else {
            eprintln!("SKIP: {target:?} runtime requires its matching host");
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
            eprintln!("SKIP: native array control execution requires the Linux/macOS host harness");
        }
    }
}

#[test]
fn narrow_integer_call_results_normalize_full_registers_with_owned_arrays() {
    for (scalar, low_bits, mask, normalized) in [
        ("u16", 65535_u64, 65535_u64, 65535_u64),
        ("i8", 255, 255, u64::MAX),
        ("i16", 32769, 65535, 18446744073709518849),
        ("i32", 2147483649, 4294967295, 18446744071562067969),
    ] {
        let source = format!(
            "machine keep(value: {scalar}, row: [u8; 2]) -> {scalar} {{ value }}
             machine forward(value: {scalar}, row: [u8; 2]) -> {scalar} {{ keep(value, row) }}
             machine selected(value: {scalar}) -> {scalar} {{
                 let first: [u8; 2] = [7u8, 9u8];
                 let returned: {scalar} = keep(value, first);
                 let second: [u8; 2] = [11u8, 13u8];
                 let forwarded: {scalar} = forward(returned, second);
                 forwarded
             }}"
        );
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            eprintln!("full register integer normalization: {scalar} on {target:?}");
            let (image, offset) = publish(&source, "selected", target);
            #[cfg(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            ))]
            if target == NativeTarget::host() {
                // The wide C result observes Omega's internal full-register
                // normalization, not a promise about foreign ABI upper bits.
                let driver = format!(
                    "#include <stdint.h>\n extern uint64_t omega_entry(uint64_t);\n
                     int main(void) {{
                         const uint64_t upper[] = {{0, UINT64_MAX, 0x123456789abcdef0ULL}};
                         for (unsigned pattern = 0; pattern < 3; ++pattern) {{
                             uint64_t argument = (upper[pattern] & ~{mask}ULL) | {low_bits}ULL;
                             if (omega_entry(argument) != {normalized}ULL) return 1;
                             if (omega_entry(argument ^ 1ULL) != ({normalized}ULL ^ 1ULL)) return 2;
                             if (omega_entry((upper[pattern] & ~{mask}ULL) | 1ULL) != 1ULL) return 3;
                         }}
                         return 0;
                     }}"
                );
                native_function::assert_c_text(&image.output().final_text_bytes, offset, &driver);
            }
            #[cfg(not(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            )))]
            {
                let _ = (image, offset, low_bits, mask, normalized);
                eprintln!("SKIP: narrow integer runtime requires the Linux/macOS host harness");
            }
        }
    }
}

#[test]
fn transitive_source_array_arguments_and_returns_reach_native_execution() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../omega/pass/modules/module_array_constant_indices/main.omg");
    let checked =
        compiler::compile_to_checked(CheckedCompileRequest::new(&path, Some("macos_arm64")))
            .unwrap();
    let entry = "transitive_computation_row";
    let artifact = terminal_production::produce_terminal_artifact(&checked, entry).unwrap();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let reloaded =
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
        let plan = target_artifact(reloaded, target)
            .unwrap_or_else(|error| panic!("owned array target lowering on {target:?}: {error:?}"));
        let (image, offset) = publish_target(plan, entry, target);
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
                r"
                #include <stdint.h>
                typedef struct { uint8_t values[2]; } Row;
                extern Row omega_entry(uint8_t);
                int main(void) {
                    for (unsigned value = 0; value < 256; ++value) {
                        Row row = omega_entry((uint8_t)value);
                        if (row.values[0] != value || row.values[1] != 9) return 1;
                    }
                    return 0;
                }
                ",
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
            eprintln!("SKIP: array argument runtime requires the Linux/macOS host harness");
        }
    }
}

#[test]
fn owned_arrays_forward_incoming_and_call_result_payloads() {
    for (shape, initializer, c_type, count, expected) in [
        ("[u8; 1]", "[255u8]", "uint8_t", 1, "{255}"),
        ("[u16; 1]", "[65535u16]", "uint16_t", 1, "{65535}"),
        (
            "[[u16; 2]; 2]",
            "[[1u16, 65535u16], [32768u16, 2u16]]",
            "uint16_t",
            4,
            "{1, 65535, 32768, 2}",
        ),
        (
            "[u64; 2]",
            "[18446744073709551615u64, 9223372036854775809u64]",
            "uint64_t",
            2,
            "{18446744073709551615ULL, 9223372036854775809ULL}",
        ),
        ("[bool; 2]", "[true, false]", "uint8_t", 2, "{1, 0}"),
    ] {
        let source = format!(
            "machine keep(row: {shape}) -> {shape} {{ row }}
             machine forward(row: {shape}) -> {shape} {{ keep(row) }}
             machine selected() -> {shape} {{
                 let row: {shape} = {initializer};
                 let copied: {shape} = keep(row);
                 forward(copied)
             }}"
        );
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            if shape == "[u64; 2]" && target == NativeTarget::windows_x64() {
                continue; // Microsoft indirect aggregate results remain a separate dependency.
            }
            eprintln!("owned array forwarding: {shape} on {target:?}");
            let (image, offset) = publish(&source, "selected", target);
            #[cfg(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            ))]
            if target == NativeTarget::host() {
                let driver = format!(
                    "#include <stdint.h>\n typedef struct {{ {c_type} values[{count}]; }} Row;\n extern Row omega_entry(void);\n int main(void) {{ const {c_type} expected[{count}] = {expected}; Row row = omega_entry(); for (unsigned element = 0; element < {count}; ++element) if (row.values[element] != expected[element]) return 1; return 0; }}"
                );
                native_function::assert_c_text(&image.output().final_text_bytes, offset, &driver);
            }
            #[cfg(not(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            )))]
            {
                let _ = (image, offset, c_type, count, expected);
                eprintln!("SKIP: array forwarding runtime requires the Linux/macOS host harness");
            }
        }
    }
}
