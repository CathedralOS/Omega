//! The complete module customer must transport owned payloads, not just construct them.
use super::*;

#[test]
fn transitive_source_array_arguments_and_returns_reach_native_execution() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../omega/pass/modules/module_array_constant_indices/main.omg");
    let checked = compiler::compile_to_checked(&path, Some("macos_arm64")).unwrap();
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
