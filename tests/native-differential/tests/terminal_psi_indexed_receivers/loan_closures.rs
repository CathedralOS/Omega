//! Closed projected loans restore the borrowed parent for later calls.

use super::{NativeTarget, primitive_stores};

/// A `&write` subloan of one element closes at its last use; the restored
/// parent then writes a disjoint element through the original referent.
#[test]
fn closed_projected_subloan_restores_the_parent() {
    for access in ["write", "mut"] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
            machine Record::replace(&write self, value: u16) {{ self.value = value; }}
            machine forward(records: &{access} [Record; 2], value: u16) {{
                let held: &write Record = &write records[1];
                held.replace(value);
                records[0].replace(4660);
            }}"
        );
        assert_record_pair_publication(&source);
    }
}

/// The restored parent reaches the same referent the closed subloan used, so
/// a later parent call on the same element overwrites the subloan's store.
#[test]
fn closed_subloan_releases_the_same_element_to_the_parent() {
    for access in ["write", "mut"] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
            machine Record::replace(&write self, value: u16) {{ self.value = value; }}
            machine Record::clear(&write self) {{ self.value = 0; }}
            machine forward(records: &{access} [Record; 2], value: u16) {{
                let held: &write Record = &write records[1];
                held.replace(value);
                records[1].clear();
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
                    frame.records[1].value = 0;
                    unsigned char expected[sizeof frame];
                    memcpy(expected, &frame, sizeof frame);
                    frame.records[1].value = 0xcccc;
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
                eprintln!("SKIP: loan closure execution requires a supported Linux or macOS host");
            }
        }
    }
}

/// A field subloan closes independently of its sibling field: the restored
/// parent writes the sibling through the original container referent.
#[test]
fn closed_field_subloan_restores_sibling_fields() {
    for access in ["write", "mut"] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
            data Container [copy] {{ record: Record; other: Record; }}
            machine Record::replace(&write self, value: u16) {{ self.value = value; }}
            machine forward(root: &{access} Container, value: u16) {{
                let held: &write Record = &write root.record;
                held.replace(value);
                root.other.replace(1877);
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
                typedef struct { Record record; Record other; } Container;
                extern void omega_entry(uint16_t value, Container *root);
                int main(void) {
                    struct { uint64_t before; Container root; uint64_t after; } frame;
                    memset(&frame, 0xa5, sizeof frame);
                    unsigned char expected[sizeof frame];
                    memcpy(expected, &frame, sizeof frame);
                    frame.root.record.value = 0xbeef;
                    frame.root.other.value = 0x0755;
                    memcpy(expected, &frame, sizeof frame);
                    omega_entry(0xbeef, &frame.root);
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
                eprintln!("SKIP: loan closure execution requires a supported Linux or macOS host");
            }
        }
    }
}

/// Two live subloans over disjoint elements close independently: each keeps
/// its exact projected referent while the other stays open.
#[test]
fn live_disjoint_subloans_close_independently() {
    for access in ["write", "mut"] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
            machine Record::replace(&write self, value: u16) {{ self.value = value; }}
            machine Record::clear(&write self) {{ self.value = 0; }}
            machine forward(records: &{access} [Record; 2], value: u16) {{
                let a: &write Record = &write records[0];
                let b: &write Record = &write records[1];
                a.replace(255);
                b.replace(value);
                a.clear();
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
                    unsigned char expected[sizeof frame];
                    memcpy(expected, &frame, sizeof frame);
                    frame.records[0].value = 0;
                    frame.records[1].value = 0xbeef;
                    memcpy(expected, &frame, sizeof frame);
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
                eprintln!("SKIP: loan closure execution requires a supported Linux or macOS host");
            }
        }
    }
}

/// Loan closure replays inside a borrowed callee: the subloan and the restored
/// parent both reach the caller's original referent across the call boundary.
#[test]
fn restored_parent_replays_inside_a_borrowed_callee() {
    for access in ["write", "mut"] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
            machine Record::replace(&write self, value: u16) {{ self.value = value; }}
            machine nested(records: &write [Record; 2], value: u16) {{
                let held: &write Record = &write records[1];
                held.replace(value);
                records[0].replace(4660);
            }}
            machine forward(records: &{access} [Record; 2], value: u16) {{
                nested(&write records, value);
            }}"
        );
        assert_record_pair_publication(&source);
    }
}

fn assert_record_pair_publication(source: &str) {
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
                unsigned char expected[sizeof frame];
                memcpy(expected, &frame, sizeof frame);
                frame.records[0].value = 0x1234;
                frame.records[1].value = 0xbeef;
                memcpy(expected, &frame, sizeof frame);
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
            eprintln!("SKIP: loan closure execution requires a supported Linux or macOS host");
        }
    }
}
