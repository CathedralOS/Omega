//! Mutable alias descent preserves original storage when the child attenuates to write-only.
use super::*;

#[test]
fn mutable_parent_alias_captures_projected_write_child_through_publication() {
    let source = "data Record [copy] { before: u8; value: u16; after: u8; }
        machine Record::replace(&write self, value: u16) { self.value = value; }
        machine forward(root: &mut [Record; 2], value: u16) {
            let parent: &mut [Record; 2] = &mut root;
            let child: &write Record = &write parent[1];
            child.replace(17);
            child.replace(value);
        }";
    assert_publication(source, "Record", "", "frame.root[1].value");
}

#[test]
fn nested_mutable_aliases_attenuate_after_static_field_and_index_projection() {
    let source = "data Record [copy] { before: u8; value: u16; after: u8; }
        data Container [copy] { before: u32; records: [Record; 2]; after: u64; }
        machine Record::replace(&write self, value: u16) { self.value = value; }
        machine forward(root: &mut [Container; 2], value: u16) {
            let parent: &mut Container = &mut root[1];
            let records: &mut [Record; 2] = &mut parent.records;
            let child: &write Record = &write records[0];
            child.replace(17);
            child.replace(value);
        }";
    assert_publication(
        source,
        "Container",
        "typedef struct { uint32_t before; Record records[2]; uint64_t after; } Container;",
        "frame.root[1].records[0].value",
    );
}

fn assert_publication(source: &str, root_type: &str, native_types: &str, destination: &str) {
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
            &format!(
                r#"
            #include <stdint.h>
            #include <string.h>
            typedef struct {{ uint8_t before; uint16_t value; uint8_t after; }} Record;
            {native_types}
            extern void omega_entry(uint16_t value, {root_type} *root);
            int main(void) {{
                uint16_t values[] = {{ 0, 0xbeef, 0xffff }};
                for (unsigned iteration = 0; iteration < 3; ++iteration) {{
                    struct {{ uint64_t before; {root_type} root[2]; uint64_t after; }} frame;
                    memset(&frame, 0xa5, sizeof frame);
                    {destination} = values[iteration];
                    unsigned char expected[sizeof frame];
                    memcpy(expected, &frame, sizeof frame);
                    {destination} = 0x5a5a;
                    omega_entry(values[iteration], frame.root);
                    if (memcmp(expected, &frame, sizeof frame) != 0) return 1;
                }}
                return 0;
            }}
            "#
            ),
        );
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        {
            let _ = (bytes, entry, root_type, native_types, destination);
            eprintln!(
                "SKIP: mutable alias caller observation requires a supported Linux or macOS host"
            );
        }
    }
}
