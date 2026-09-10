//! Empty arrays retain typed call custody without physical payload storage.
use super::*;

#[test]
fn empty_arrays_return_without_a_physical_payload() {
    for (shape, value) in [
        ("[u8; 0]", "[]"),
        ("[f64; 0]", "[]"),
        ("[[u64; 3]; 0]", "[]"),
        ("[[u8; 0]; 3]", "[[], [], []]"),
    ] {
        let source = format!("machine selected() -> {shape} {{ {value} }}");
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            publish(&source, "selected", target);
        }
    }
}

#[test]
fn empty_result_calls_preserve_observable_writes() {
    let source = "
        machine write(output: &mut u8) -> [u8; 0] { output = 17u8; [] }
        machine keep(row: [u8; 0]) -> [u8; 0] { row }
        machine selected(output: &mut u8) -> [u8; 0] {
            let before: [u8; 0] = [];
            let again: [u8; 0] = keep(before);
            let after: [u8; 0] = write(output);
            keep(after)
        }
    ";
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
                "#include <stdint.h>
                 extern void omega_entry(uint8_t*);
                 int main(void) {
                     uint8_t value = 0;
                     omega_entry(&value);
                     return value == 17 ? 0 : 1;
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
            eprintln!("SKIP: empty-result runtime requires the Linux/macOS host harness");
        }
    }
}

#[test]
fn empty_arrays_preserve_typed_construction_and_transitive_calls() {
    let source = "
        machine keep(row: [u8; 0]) -> [u8; 0] { row }
        machine forward(row: [u8; 0]) -> [u8; 0] { keep(row) }
        machine make() -> [u8; 0] { [] }
        machine consume(row: [u8; 0], value: u8) -> u8 { value }
        machine selected(value: u8) -> u8 {
            let row: [u8; 0] = make();
            let copied: [u8; 0] = forward(row);
            consume(copied, value)
        }
    ";
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
                "#include <stdint.h>
                 extern uint8_t omega_entry(uint8_t);
                 int main(void) {
                     for (unsigned value = 0; value < 256; ++value)
                         if (omega_entry(value) != value) return 1;
                     return 0;
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
            eprintln!("SKIP: empty-array runtime requires the Linux/macOS host harness");
        }
    }
}
