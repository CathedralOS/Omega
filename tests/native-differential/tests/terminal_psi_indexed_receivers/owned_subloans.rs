//! Owned callers observe writes through projected exclusive subloans.

use super::{NativeTarget, native_function, native_text};

#[test]
fn owned_record_field_subloans_restore_caller_visible_writes() {
    for access in ["mut", "write"] {
        let source = format!(
            "data Record [copy] {{ value: u64; }}
             data Container [copy] {{ record: Record; untouched: u64; }}
             machine Record::replace(&{access} self, value: u64) {{ self.value = value; }}
             machine update(mut container: Container, value: u64, output: &mut u64, untouched: &mut u64) {{
                 container.record.replace(value);
                 output = container.record.value;
                 untouched = container.untouched;
             }}
             machine forward(value: u64, output: &mut u64, untouched: &mut u64) {{
                 let container: Container = Container {{ record: Record {{ value: 201 }}, untouched: 99 }};
                 update(container, value, output, untouched);
             }}"
        );
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            // This exercises independently checked Terminal and physical text,
            // not installation-record publication, whose owned-parent call
            // replay remains a separate STRUCTURAL-BORROW-IDENTITY dependency.
            let placed = native_text(&source, target);
            let text = placed.text_section();
            let entry = text
                .functions
                .iter()
                .find(|function| function.machine == text.semantic_entry)
                .expect("native entry");
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
                &text.bytes,
                entry
                    .section_offset
                    .try_into()
                    .expect("native entry offset"),
                r#"
                #include <stdint.h>
                typedef struct { uint64_t value; uint64_t untouched; } Container;
                extern void omega_entry(uint64_t value, uint64_t *output, uint64_t *untouched);
                int main(void) {
                    struct { uint64_t before; Container output; uint64_t after; } frame =
                        { 0x1234, {0, 0}, 0x5678 };
                    omega_entry(41, &frame.output.value, &frame.output.untouched);
                    if (frame.output.value != 41 || frame.output.untouched != 99 ||
                        frame.before != 0x1234 || frame.after != 0x5678) return 1;
                    omega_entry(77, &frame.output.value, &frame.output.untouched);
                    return frame.output.value != 77 || frame.output.untouched != 99 ||
                        frame.before != 0x1234 || frame.after != 0x5678;
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
                let _ = (text, entry);
                eprintln!(
                    "SKIP: owned projected subloan execution requires a supported Linux or macOS host"
                );
            }
        }
    }
}
