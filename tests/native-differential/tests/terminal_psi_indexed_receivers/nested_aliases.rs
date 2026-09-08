//! Receiver alias erasure must replay every immediate parent before publication.
use super::*;

const NESTED_RECEIVER: &str = "data Record [copy] { value: u16; }
    machine Record::replace(&write self, value: u16) { self.value = value; }
    machine forward(records: &write [Record; 2], value: u16) {
        let held: &write [Record; 2] = &write records;
        let child: &write [Record; 2] = &write held;
        child[1].replace(value);
    }";

#[test]
fn nested_write_only_receiver_retains_original_storage_through_publication() {
    let wrapped = format!(
        "{}\nmachine forward(records: &write [Record; 2], value: u16) {{
            nested(&write records, value);
        }}",
        NESTED_RECEIVER.replace("machine forward(", "machine nested(")
    );
    for source in [NESTED_RECEIVER, wrapped.as_str()] {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let (bytes, entry) = primitive_stores::published_text(source, target);
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
            native_function::assert_c_text(
                &bytes,
                entry,
                r#"
            #include <stdint.h>
            #include <string.h>
            typedef struct { uint16_t value; } Record;
            extern void omega_entry(uint16_t value, Record *records);
            int main(void) {
                struct { uint64_t before; Record records[2]; uint64_t after; } frame;
                memset(&frame, 0xa5, sizeof frame);
                frame.records[1].value = 0xbeef;
                unsigned char expected[sizeof frame];
                memcpy(expected, &frame, sizeof frame);
                frame.records[1].value = 0;
                omega_entry(0xbeef, frame.records);
                return memcmp(expected, &frame, sizeof frame) != 0;
            }
        "#,
            );
            #[cfg(not(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            )))]
            {
                let _ = (bytes, entry);
                eprintln!(
                    "SKIP: nested receiver execution requires a supported Linux or macOS host"
                );
            }
        }
    }
}

#[test]
fn deeper_receiver_chains_keep_attenuation_and_ordered_writes() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    for access in ["write", "mut"] {
        for depth in [3, 7] {
            let mut prefix = String::new();
            for position in 0..depth {
                let parent = if position == 0 {
                    "records".to_owned()
                } else {
                    format!("held{}", position - 1)
                };
                prefix.push_str(&format!(
                    "let held{position}: &write [Record; 2] = &write {parent};\n"
                ));
            }
            let leaf = depth - 1;
            let source = format!(
                "data Record [copy] {{ value: u16; }}
                machine Record::replace(&write self, value: u16) {{ self.value = value; }}
                machine forward(records: &{access} [Record; 2], value: u16) {{
                    {prefix}
                    held{leaf}[0].replace(4660);
                    held{leaf}[1].replace(value);
                }}"
            );
            let (bytes, entry) = primitive_stores::published_text(&source, NativeTarget::host());
            native_function::assert_c_text(
                &bytes,
                entry,
                r#"
                #include <stdint.h>
                #include <string.h>
                typedef struct { uint16_t value; } Record;
                extern void omega_entry(uint16_t value, Record *records);
                int main(void) {
                    struct { uint64_t before; Record records[2]; uint64_t after; } frame;
                    memset(&frame, 0xa5, sizeof frame);
                    frame.records[0].value = 0x1234;
                    frame.records[1].value = 0xbeef;
                    unsigned char expected[sizeof frame];
                    memcpy(expected, &frame, sizeof frame);
                    frame.records[0].value = 0;
                    frame.records[1].value = 0;
                    omega_entry(0xbeef, frame.records);
                    return memcmp(expected, &frame, sizeof frame) != 0;
                }
            "#,
            );
        }
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: nested receiver execution requires a supported Linux or macOS host");
}
