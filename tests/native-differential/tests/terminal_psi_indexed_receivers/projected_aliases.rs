//! Captured alias projections preserve the original caller storage.
use super::*;

const PROJECTED_RECEIVER: &str = "data Record [copy] { value: u16; }
    machine Record::replace(&write self, value: u16) { self.value = value; }
    machine forward(records: &write [Record; 2], value: u16) {
        let held: &write Record = &write records[1];
        held.replace(value);
    }";

#[test]
fn projected_receiver_alias_publishes_and_updates_original_storage() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (bytes, entry) = primitive_stores::published_text(PROJECTED_RECEIVER, target);
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
            eprintln!("SKIP: projected alias execution requires a supported Linux or macOS host");
        }
    }
}

#[test]
fn projected_alias_prefixes_compose_with_nested_carriers_and_receiver_suffixes() {
    let bodies = [
        "let held: &write Record = &write containers[1].records[0];
         held.replace(value);",
        "let held: &write [Record; 2] = &write containers[1].records;
         held[0].replace(value);",
        "let held: &write Container = &write containers[1];
         held.records[0].replace(value);",
        "let held: &write Container = &write containers[1];
         let records: &write [Record; 2] = &write held.records;
         let child: &write Record = &write records[0];
         child.replace(value);",
        "let held: &write [Record; 2] = &write containers[1].records;
         let child: &write [Record; 2] = &write held;
         child[0].replace(value);",
        "let held: &write [Container; 2] = &write containers;
         let child: &write Record = &write held[1].records[0];
         child.replace(value);",
    ];
    for access in ["write", "mut"] {
        for body in bodies {
            let source = format!(
                "data Record [copy] {{ value: u16; }}
                 data Container [copy] {{ record: Record; records: [Record; 2]; }}
                 machine Record::replace(&write self, value: u16) {{ self.value = value; }}
                 machine forward(containers: &{access} [Container; 2], value: u16) {{ {body} }}"
            );
            assert_container_publication(&source);
        }
    }
}

#[test]
fn projected_self_alias_keeps_attachment_and_called_machine_custody() {
    let source = "data Record [copy] { value: u16; }
        data Container [copy] { record: Record; records: [Record; 2]; }
        machine Record::replace(&write self, value: u16) { self.value = value; }
        machine Container::capture(&write self, value: u16) {
            let held: &write [Record; 2] = &write self.records;
            let child: &write Record = &write held[0];
            child.replace(value);
        }
        machine forward(containers: &write [Container; 2], value: u16) {
            containers[1].capture(value);
        }";
    assert_container_publication(source);
    assert_container_publication(&source.replace("self.records", "records"));
}

fn assert_container_publication(source: &str) {
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
            typedef struct { Record record; Record records[2]; } Container;
            extern void omega_entry(uint16_t value, Container *containers);
            int main(void) {
                struct { uint64_t before; Container containers[2]; uint64_t after; } frame;
                memset(&frame, 0xa5, sizeof frame);
                frame.containers[1].records[0].value = 0xbeef;
                unsigned char expected[sizeof frame];
                memcpy(expected, &frame, sizeof frame);
                frame.containers[1].records[0].value = 0;
                omega_entry(0xbeef, frame.containers);
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
            eprintln!("SKIP: projected alias execution requires a supported Linux or macOS host");
        }
    }
}
