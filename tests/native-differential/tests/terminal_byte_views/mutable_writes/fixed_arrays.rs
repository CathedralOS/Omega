//! Fixed-array loans preserve backing independently of descriptor ABI size.
use super::*;

#[path = "fixed_arrays/admission.rs"]
mod admission;

fn array_writer(length: usize, field: bool) -> lowered_psi::LoweredPsi {
    let caller = if field {
        format!(
            "data Buffers {{ before: u8; bytes: [u8; {length}]; other: [u8; {length}]; }}\n\
             data Record {{ before: u8; buffers: Buffers; after: u64; }}\n\
             machine Record::run(&mut self, byte: u8) {{\n\
                 fill(&mut self.buffers.bytes, 0);\n\
                 fill(&mut self.buffers.bytes, byte);\n\
             }}"
        )
    } else {
        format!(
            "machine run(bytes: &mut [u8; {length}], byte: u8) {{ fill(bytes, 0); fill(bytes, byte); }}"
        )
    };
    lower_writer(
        &format!("{FILL}\n{caller}"),
        if field { "Record::run" } else { "run" },
    )
}

#[test]
fn fixed_array_loans_publish_whole_and_nested_fields_on_hosted_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for field in [false, true] {
            let lowered = array_writer(3, field);
            let _published = publish_lowered(target, &lowered);
        }
    }
}

#[test]
fn fixed_array_descriptor_crosses_outgoing_stack_arguments() {
    let fill = FILL.replacen(
        "out: &mut [u8], byte: u8",
        "out: &mut [u8], byte: u8, first: u64, second: u64, third: u64, fourth: u64, fifth: u64, sixth: u64, seventh: u64, eighth: u64",
        1,
    );
    let source = format!(
        "{fill}\n machine run(bytes: &mut [u8; 3], byte: u8) {{\n\
        fill(bytes, byte, 1, 2, 3, 4, 5, 6, 7, 8);\n\
        fill(bytes, byte, 1, 2, 3, 4, 5, 6, 7, 8);\n\
    }}"
    );
    let lowered = lower_writer(&source, "run");
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (image, entry_offset) = publish_lowered(target, &lowered);
        if target == NativeTarget::host() {
            #[cfg(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64"),
            ))]
            native_function::assert_c_text(
                &image.output().final_text_bytes,
                entry_offset,
                r#"
                #include <stdint.h>
                #include <string.h>
                #include <unistd.h>
                extern void omega_entry(uint8_t byte, uint8_t *bytes);
                int main(void) {
                    alarm(10);
                    uint8_t bytes[] = {0xa5, 0, 0, 0, 0x5a};
                    const uint8_t expected[] = {0xa5, 0xff, 0xff, 0xff, 0x5a};
                    omega_entry(0xff, bytes + 1);
                    omega_entry(0xff, bytes + 1);
                    return memcmp(bytes, expected, sizeof(bytes)) != 0;
                }
            "#,
            );
            #[cfg(not(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64"),
            )))]
            {
                let _ = (image, entry_offset);
                eprintln!(
                    "SKIP: stack-passed array descriptor execution needs a supported native C/text host"
                );
            }
        }
    }
}

#[test]
fn fixed_array_loans_execute_repeated_calls_without_changing_siblings_or_padding() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    for length in [1, 3, 17] {
        for field in [false, true] {
            let lowered = array_writer(length, field);
            let (image, entry_offset) = publish_lowered(NativeTarget::host(), &lowered);
            let declarations = if field {
                format!(
                    "struct Buffers {{ uint8_t before; uint8_t bytes[{length}]; uint8_t other[{length}]; }};\n\
                     struct Record {{ uint8_t before; struct Buffers buffers; uint64_t after; }};\n\
                     extern void omega_entry(uint8_t byte, struct Record *record);"
                )
            } else {
                format!(
                    "struct Record {{ uint8_t before; uint8_t bytes[{length}]; uint8_t after; }};\n\
                     extern void omega_entry(uint8_t byte, uint8_t *bytes);"
                )
            };
            let bytes = if field { "buffers.bytes" } else { "bytes" };
            let argument = if field { "&actual" } else { "actual.bytes" };
            let driver = format!(
                r#"
                #include <stdint.h>
                #include <string.h>
                #include <unistd.h>
                {declarations}
                int main(void) {{
                    alarm(10);
                    for (unsigned byte = 0; byte <= 255; ++byte) {{
                        struct Record actual, expected;
                        memset(&actual, 0x5a, sizeof(actual));
                        memcpy(&expected, &actual, sizeof(actual));
                        memset(expected.{bytes}, byte, {length});
                        omega_entry((uint8_t)byte, {argument});
                        if (memcmp(&actual, &expected, sizeof(actual))) return 1;
                        omega_entry((uint8_t)byte, {argument});
                        if (memcmp(&actual, &expected, sizeof(actual))) return 2;
                    }}
                    return 0;
                }}
            "#
            );
            native_function::assert_c_text(&image.output().final_text_bytes, entry_offset, &driver);
        }
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!("SKIP: fixed-array caller execution needs a matching supported host and C linker");
}
