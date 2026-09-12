//! Record construction uses runtime operands and the ordinary aggregate return.

use super::*;

const DIRECT_LOCAL_RECORD_GETTER: &str = "
    data Pair [copy] { left: u64; right: u64; }
    machine Pair::get_right(&self) -> u64 { self.right }
    machine direct_local(left: u64, right: u64) -> u64 {
        let prefix: u64 = left & 255;
        let local: Pair = Pair { right: right, left: left };
        let observed: u64 = local.get_right();
        observed ^ prefix
    }
    machine direct_tail(left: u64, right: u64) -> u64 {
        let prefix: u64 = left & 255;
        let local: Pair = Pair { left: left, right: right ^ prefix };
        local.get_right()
    }
";

const RETURNED_LOCAL_RECORD_GETTER: &str = "
    data Region { base: u64; length: u64; }
    machine Region::new(base: u64, length: u64) -> Region {
        Region { base: base, length: length }
    }
    machine Region::get_length(&self) -> u64 { self.length }
    machine returned_local(left: u64, right: u64) -> u64 {
        let prefix: u64 = left & 255;
        let local: Region = Region::new(left, right);
        let observed: u64 = local.get_length();
        observed ^ prefix
    }
    machine combine(observed: u64, prefix: u64) -> u64 { observed ^ prefix }
    machine nested_getter(left: u64, right: u64) -> u64 {
        let prefix: u64 = left & 255;
        let local: Region = Region::new(left, right);
        combine(local.get_length(), prefix)
    }
";

fn assert_local_record_getter(entry: &str, source: &str) {
    eprintln!("source-to-native local record getter: {entry}");
    let artifact = produce_source(entry, source);
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
            native_function::assert_c_text(
                &image.output().final_text_bytes,
                offset,
                r#"
                #include <stdint.h>
                extern uint64_t omega_entry(uint64_t left, uint64_t right);
                int main(void) {
                    const uint64_t inputs[][2] = {
                        {0, UINT64_MAX},
                        {UINT64_MAX, 0},
                        {UINT64_C(0x8123456789abcdef), UINT64_C(0xfedcba9876543210)},
                        {7, 19}
                    };
                    for (unsigned iteration = 0; iteration < 4; ++iteration) {
                        uint64_t left = inputs[iteration][0];
                        uint64_t right = inputs[iteration][1];
                        if (omega_entry(left, right) != (right ^ (left & 255))) return 1;
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
                "SKIP: local record getter execution needs a matching Linux/macOS host; cross-target publication was checked"
            );
        }
    }
}

#[test]
fn direct_runtime_record_local_shared_getter_preserves_full_width_value() {
    for entry in ["direct_local", "direct_tail"] {
        assert_local_record_getter(entry, DIRECT_LOCAL_RECORD_GETTER);
    }
}

#[test]
fn returned_runtime_record_local_shared_getter_preserves_full_width_value() {
    for entry in ["returned_local", "nested_getter"] {
        assert_local_record_getter(entry, RETURNED_LOCAL_RECORD_GETTER);
    }
}

#[test]
fn record_wire_rejects_retired_literal_tag_and_changed_field_roster() {
    let artifact = produce_source(
        "construct",
        "data Pair [copy] { left: u64; right: u64; }
        machine construct(left: u64, right: u64) -> Pair { Pair { left: left, right: right } }",
    );
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let fields = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::EstablishScalarRecord { fields } => Some(fields),
            _ => None,
        })
        .unwrap();
    // Pin the new row's bytes so tag 51 can never silently acquire new meaning.
    let mut row = vec![67];
    row.extend_from_slice(&u32::try_from(fields.len()).unwrap().to_le_bytes());
    for field in fields {
        row.extend_from_slice(&field.field.get().to_le_bytes());
        row.extend_from_slice(&field.value.get().to_le_bytes());
    }
    let offsets = artifact
        .semantic_bytes()
        .windows(row.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == row).then_some(offset))
        .collect::<Vec<_>>();
    let [offset] = offsets.as_slice() else {
        panic!("one exact record wire row");
    };
    let mut stale = artifact.semantic_bytes().to_vec();
    stale[*offset] = 51;
    assert!(terminal_codec::decode_module(&stale).is_err());
    let mut changed = module.clone();
    let fields = changed
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            terminal_psi::OperationKind::EstablishScalarRecord { fields } => Some(fields),
            _ => None,
        })
        .unwrap();
    fields.swap(0, 1);
    assert!(terminal_codec::encode_module(&changed).is_err());
}

#[test]
fn record_field_calls_execute_once_in_authored_order() {
    let source = "data Pair [copy] { left: u64; right: u64; }
        machine replace(value: &mut u64, replacement: u64) -> u64 {
            let before: u64 = value;
            value = replacement;
            before
        }
        machine construct(value: &mut u64) -> Pair {
            Pair { right: replace(&mut value, 91), left: replace(&mut value, 123) }
        }";
    let artifact = produce_source("construct", source);
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
            native_function::assert_c_text(
                &image.output().final_text_bytes,
                offset,
                r#"
                #include <stdint.h>
                typedef struct { uint64_t left; uint64_t right; } Pair;
                extern Pair omega_entry(uint64_t *value);
                int main(void) {
                    uint64_t value = UINT64_MAX;
                    Pair result = omega_entry(&value);
                    return result.right != UINT64_MAX || result.left != 91 || value != 123;
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
                "SKIP: record-return execution needs a matching Linux/macOS host; cross-target publication was checked"
            );
        }
    }
}

#[test]
fn scalar_record_return_preserves_field_identity_and_full_width_payload() {
    let source = "data Pair [copy] { left: u64; right: i64; }
        machine construct(left: u64, right: i64) -> Pair {
            let prefix: u64 = left & 255;
            Pair { right: right, left: left }
        }";
    let artifact = produce_source("construct", source);
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
            native_function::assert_c_text(
                &image.output().final_text_bytes,
                offset,
                r#"
                #include <stdint.h>
                typedef struct { uint64_t left; int64_t right; } Pair;
                extern Pair omega_entry(uint64_t left, int64_t right);
                int main(void) {
                    Pair first = omega_entry(UINT64_MAX, INT64_MIN);
                    Pair second = omega_entry(37, -91);
                    return first.left != UINT64_MAX || first.right != INT64_MIN ||
                           second.left != 37 || second.right != -91;
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
                "SKIP: direct record-return execution needs a matching Linux/macOS host; cross-target publication was checked"
            );
        }
    }
}

#[test]
fn heterogeneous_scalar_record_preserves_boolean_and_ieee_bits() {
    let source = "data Reading [copy] { word: u32; flag: bool; real: f32; }
        machine construct(word: u32, flag: bool, real: f32) -> Reading {
            Reading { real: real, word: word, flag: flag }
        }";
    let artifact = produce_source("construct", source);
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
            native_function::assert_c_text(
                &image.output().final_text_bytes,
                offset,
                r#"
                #include <stdint.h>
                #include <stdbool.h>
                #include <string.h>
                typedef struct { uint32_t word; bool flag; float real; } Reading;
                extern Reading omega_entry(uint32_t word, bool flag, float real);
                int main(void) {
                    const uint32_t bits[] = {0x80000000, 0x7fc12345, 0x3fc00000};
                    for (unsigned iteration = 0; iteration < 3; ++iteration) {
                        float real;
                        memcpy(&real, &bits[iteration], sizeof real);
                        Reading value = omega_entry(UINT32_MAX, iteration != 0, real);
                        uint32_t observed;
                        memcpy(&observed, &value.real, sizeof observed);
                        if (value.word != UINT32_MAX || value.flag != (iteration != 0) ||
                            observed != bits[iteration]) return 1;
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
                "SKIP: record-return execution needs a matching Linux/macOS host; cross-target publication was checked"
            );
        }
    }
}
