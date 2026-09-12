//! State-local record storage ends after its getter observation, on either exit.
use super::{NativeTarget, produce_source, publish};

const SOURCE: &str = r#"
    data Region { value: u64; }
    data Filter { region: Region; }
    machine Filter::new(value: u64) -> Filter {
        Filter { region: Region { value: value } }
    }
    machine Region::get(&self) -> u64 { self.value }
    machine Filter::get(&self) -> u64 { self.region.get() }
    data Main {}
    machine Main::main(input: u64, out: &mut [u8]) {
        let local: Filter = Filter::new(input);
        let observed: u64 = local.get();
        transition observed == 7 {
            true -> selected(out, observed, input)
            false -> other(out, observed, input)
        }
        state selected(out: &mut [u8], observed: u64, expected: u64) {
            transition observed == expected {
                true -> writable(out, 11)
                false -> writable(out, 255)
            }
        }
        state other(out: &mut [u8], observed: u64, expected: u64) {
            transition observed == expected {
                true -> writable(out, 22)
                false -> writable(out, 255)
            }
        }
        state writable(out: &mut [u8], marker: u8) {
            transition out.len > 0 { true -> store(out, marker) false -> done() }
        }
        state store(out: &mut [u8], marker: u8) { out[0] = marker; }
        state done() {}
    }
"#;

#[test]
fn state_local_nested_record_getter_survives_conditional_disposal() {
    let artifact = produce_source("Main::main", SOURCE);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let locals = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| operation.result.structural())
        .collect::<Vec<_>>();
    let [local] = locals.as_slice() else {
        panic!("one state-local Filter result");
    };
    let disposals = entry
        .blocks
        .iter()
        .map(|block| match &block.terminator {
            terminal_psi::Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => when_true
                .trivial_affine_discards
                .iter()
                .chain(&when_false.trivial_affine_discards)
                .filter(|place| **place == local.place)
                .count(),
            terminal_psi::Terminator::Jump {
                trivial_affine_discards,
                ..
            } => trivial_affine_discards
                .iter()
                .filter(|place| **place == local.place)
                .count(),
            _ => 0,
        })
        .sum::<usize>();
    assert_eq!(
        disposals, 2,
        "both selected routes dispose the local after reading it"
    );

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (image, offset) = publish(&artifact, target);
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        if target == NativeTarget::host() {
            super::native_function::assert_c_text(
                &image.output().final_text_bytes,
                offset,
                r#"
                #include <stdint.h>
                #include <stddef.h>
                #include <string.h>
                struct byte_view { uint8_t *bytes; uint64_t length; };
                extern void omega_entry(uint64_t input, struct byte_view *out);
                int main(void) {
                    const uint64_t inputs[] = {0, 7, UINT64_C(0x123456789abcdef0), UINT64_MAX};
                    for (unsigned repeat = 0; repeat < 3; ++repeat) {
                        for (size_t index = 0; index < sizeof(inputs) / sizeof(inputs[0]); ++index) {
                            for (uint64_t length = 0; length < 4; ++length) {
                                uint8_t bytes[6];
                                memset(bytes, 0xa7, sizeof(bytes));
                                struct byte_view out = {bytes + 1, length};
                                omega_entry(inputs[index], &out);
                                if (out.bytes != bytes + 1 || out.length != length) return 1;
                                for (size_t position = 0; position < sizeof(bytes); ++position) {
                                    uint8_t expected = length && position == 1
                                        ? (inputs[index] == 7 ? 11 : 22) : 0xa7;
                                    if (bytes[position] != expected) return 2;
                                }
                            }
                        }
                    }
                    return 0;
                }
            "#,
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
            eprintln!(
                "SKIP: state-local record runtime requires a matching Linux or macOS ARM64 host; cross-target publication was checked"
            );
        }
    }
}
