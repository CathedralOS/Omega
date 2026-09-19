//! Borrowed arguments beyond receivers keep their referents across ordinary
//! calls: disjoint write-only roots, mixed-access field borrows of one
//! aggregate, owned local roots, callee re-forwarding, and attached callees
//! with extra write-only parameters all land on caller storage.

use super::{NativeTarget, artifact_for, native_function, native_text_for, optimize};

/// Two disjoint `&write` arguments in one call keep their own referents.
const DISJOINT_WRITE_ONLY: &str = "machine stamp(left: &write u64, right: &write u64, value: u64) {
    left = value;
    right = value;
}
machine forward(first: &mut u64, second: &mut u64, value: u64) {
    stamp(&write first, &write second, value);
}";

/// One aggregate can lend disjoint fields at different access in one call.
const MIXED_ACCESS_FIELDS: &str = "data Pair [copy] { left: u64; right: u64; }
machine stamp(left: &write u64, right: &mut u64, first: u64, second: u64) {
    left = first;
    right = second;
}
machine forward(pair: &mut Pair, first: u64, second: u64) {
    stamp(&write pair.left, &mut pair.right, first, second);
}";

/// An owned local root lends `&write` to a call; the restored local reads back
/// the written value on return.
const LOCAL_ROOT_WRITE_ONLY: &str = "machine stamp(slot: &write u64, value: u64) { slot = value; }
machine enter(value: u64) -> u64 {
    let mut slot: u64 = 201;
    stamp(&write slot, value);
    slot
}";

/// The same owned local root lends `&mut` to a call.
const LOCAL_ROOT_MUTABLE: &str = "machine stamp(slot: &mut u64, value: u64) { slot = value; }
machine enter(value: u64) -> u64 {
    let mut slot: u64 = 201;
    stamp(&mut slot, value);
    slot
}";

/// A write-only parameter re-forwards to a deeper call with explicit `&write`
/// attenuation; the original caller referent is still what gets written.
const WRITE_ONLY_REFORWARD: &str = "machine inner(out: &write u64, value: u64) { out = value; }
machine mid(out: &write u64, value: u64) { inner(&write out, value); }
machine forward(root: &mut u64, value: u64) {
    mid(&write root, value);
}";

/// A mutable parameter forwards to a deeper call unchanged.
const MUTABLE_FORWARD: &str = "machine inner(out: &mut u64, value: u64) { out = value; }
machine mid(out: &mut u64, value: u64) { inner(out, value); }
machine forward(root: &mut u64, value: u64) {
    mid(root, value);
}";

/// An attached callee takes a write-only argument beside `&mut self`.
const ATTACHED_WRITE_ONLY_ARGUMENT: &str = "data Record [copy] { value: u16; }
machine Record::push(&mut self, out: &write u16) { out = self.value; }
machine forward(record: &mut Record, slot: &mut u16) {
    record.push(&write slot);
}";

/// A shared `&` argument's callee body reads through to the caller's
/// referent; the entry returns the read value unchanged.
const SHARED_SCALAR_CALLEE_BODY: &str = "machine read(root: &u64) -> u64 { root }
machine forward(root: &mut u64) -> u64 { read(root) }";

pub(super) fn published_for(source: &str, entry: &str, target: NativeTarget) -> (Vec<u8>, usize) {
    let placed = native_text_for(source, target, entry);
    let entry = placed.text_section().semantic_entry;
    let container = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(placed).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(container.clone())
        .unwrap_or_else(|error| panic!("{target:?}: object publication: {error:?}\n{source}"));
    image_emission::validate_function_fragment_object_artifact(&container, &object).unwrap();
    let entry_offset = object
        .functions()
        .iter()
        .find(|function| function.machine == entry)
        .unwrap()
        .text_offset;
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    let installed = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let bytes = image_emission::encode_installation_record(&installed).unwrap();
    let decoded = image_emission::decode_installation_record(&bytes).unwrap();
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    (image.output().final_text_bytes.clone(), entry_offset)
}

pub(super) fn hosted_targets() -> [NativeTarget; 4] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ]
}

#[test]
fn borrowed_arguments_preserve_optimizer_contracts() {
    for (source, entry) in [
        (DISJOINT_WRITE_ONLY, "forward"),
        (MIXED_ACCESS_FIELDS, "forward"),
        (LOCAL_ROOT_WRITE_ONLY, "enter"),
        (LOCAL_ROOT_MUTABLE, "enter"),
        (WRITE_ONLY_REFORWARD, "forward"),
        (MUTABLE_FORWARD, "forward"),
        (ATTACHED_WRITE_ONLY_ARGUMENT, "forward"),
        (SHARED_SCALAR_CALLEE_BODY, "forward"),
    ] {
        let optimized = optimize(&artifact_for(source, entry));
        assert_eq!(optimized.plan(), optimized.verified_input().plan());
        assert_eq!(
            optimized.validation().initial_unit(),
            optimized.validation().final_unit()
        );
        assert!(optimized.commits().is_empty());
    }
}

#[test]
fn disjoint_write_only_arguments_update_distinct_caller_roots() {
    for target in hosted_targets() {
        let (bytes, entry_offset) = published_for(DISJOINT_WRITE_ONLY, "forward", target);
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
            entry_offset,
            r#"
            #include <stdint.h>
            #include <string.h>
            extern void omega_entry(uint64_t value, uint64_t *first, uint64_t *second);
            int main(void) {
                struct {
                    uint64_t pad0; uint64_t first;
                    uint64_t pad1; uint64_t second;
                    uint64_t pad2;
                } frame;
                memset(&frame, 0xa5, sizeof frame);
                __typeof__(frame) expected = frame;
                expected.first = 7;
                expected.second = 7;
                omega_entry(7, &frame.first, &frame.second);
                return memcmp(&expected, &frame, sizeof frame) != 0;
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
            let _ = (bytes, entry_offset);
            eprintln!("SKIP: disjoint write-only execution requires a supported host");
        }
    }
}

#[test]
fn mixed_access_field_arguments_update_distinct_fields() {
    for target in hosted_targets() {
        let (bytes, entry_offset) = published_for(MIXED_ACCESS_FIELDS, "forward", target);
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
            entry_offset,
            r#"
            #include <stdint.h>
            #include <string.h>
            extern void omega_entry(uint64_t first, uint64_t second, uint64_t *pair);
            int main(void) {
                struct {
                    uint64_t before;
                    uint64_t left; uint64_t right;
                    uint64_t after;
                } frame;
                memset(&frame, 0xa5, sizeof frame);
                __typeof__(frame) expected = frame;
                expected.left = 7;
                expected.right = 9;
                omega_entry(7, 9, &frame.left);
                return memcmp(&expected, &frame, sizeof frame) != 0;
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
            let _ = (bytes, entry_offset);
            eprintln!("SKIP: mixed-access field execution requires a supported host");
        }
    }
}

#[test]
fn local_root_arguments_return_the_restored_local() {
    for (source, value) in [
        (LOCAL_ROOT_WRITE_ONLY, 41_u64),
        (LOCAL_ROOT_MUTABLE, 77_u64),
    ] {
        for target in hosted_targets() {
            let (bytes, entry_offset) = published_for(source, "enter", target);
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
                entry_offset,
                &format!(
                    r#"
                #include <stdint.h>
                extern uint64_t omega_entry(uint64_t value);
                int main(void) {{
                    return omega_entry({value}ull) == {value}ull ? 0 : 1;
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
                let _ = (bytes, entry_offset);
                eprintln!("SKIP: local-root execution requires a supported host");
            }
        }
    }
}

#[test]
fn write_only_parameter_reforwards_to_the_original_caller_referent() {
    for target in hosted_targets() {
        let (bytes, entry_offset) = published_for(WRITE_ONLY_REFORWARD, "forward", target);
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
            entry_offset,
            r#"
            #include <stdint.h>
            #include <string.h>
            extern void omega_entry(uint64_t value, uint64_t *root);
            int main(void) {
                struct { uint64_t before; uint64_t root; uint64_t after; } frame;
                memset(&frame, 0xa5, sizeof frame);
                __typeof__(frame) expected = frame;
                expected.root = 3;
                omega_entry(3, &frame.root);
                return memcmp(&expected, &frame, sizeof frame) != 0;
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
            let _ = (bytes, entry_offset);
            eprintln!("SKIP: reforwarding execution requires a supported host");
        }
    }
}

#[test]
fn mutable_parameter_forwards_to_the_original_caller_referent() {
    for target in hosted_targets() {
        let (bytes, entry_offset) = published_for(MUTABLE_FORWARD, "forward", target);
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
            entry_offset,
            r#"
            #include <stdint.h>
            #include <string.h>
            extern void omega_entry(uint64_t value, uint64_t *root);
            int main(void) {
                struct { uint64_t before; uint64_t root; uint64_t after; } frame;
                memset(&frame, 0xa5, sizeof frame);
                __typeof__(frame) expected = frame;
                expected.root = 5;
                omega_entry(5, &frame.root);
                return memcmp(&expected, &frame, sizeof frame) != 0;
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
            let _ = (bytes, entry_offset);
            eprintln!("SKIP: mutable forwarding execution requires a supported host");
        }
    }
}

#[test]
fn attached_callee_write_only_argument_reaches_caller_storage() {
    for target in hosted_targets() {
        let (bytes, entry_offset) = published_for(ATTACHED_WRITE_ONLY_ARGUMENT, "forward", target);
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
            entry_offset,
            r#"
            #include <stdint.h>
            #include <string.h>
            extern void omega_entry(uint16_t *record, uint16_t *slot);
            int main(void) {
                struct { uint64_t before; uint16_t record; uint16_t slot; uint64_t after; }
                    frame;
                memset(&frame, 0xa5, sizeof frame);
                frame.record = 17;
                __typeof__(frame) expected = frame;
                expected.slot = 17;
                omega_entry(&frame.record, &frame.slot);
                return memcmp(&expected, &frame, sizeof frame) != 0;
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
            let _ = (bytes, entry_offset);
            eprintln!("SKIP: attached-callee execution requires a supported host");
        }
    }
}

#[test]
fn shared_borrow_callee_body_reads_the_caller_referent() {
    for target in hosted_targets() {
        let (bytes, entry_offset) = published_for(SHARED_SCALAR_CALLEE_BODY, "forward", target);
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
            entry_offset,
            r#"
            #include <stdint.h>
            #include <string.h>
            extern uint64_t omega_entry(uint64_t *root);
            int main(void) {
                struct { uint64_t before; uint64_t root; uint64_t after; } frame;
                memset(&frame, 0xa5, sizeof frame);
                frame.root = 41;
                __typeof__(frame) expected = frame;
                uint64_t returned = omega_entry(&frame.root);
                return returned != 41 || memcmp(&expected, &frame, sizeof frame) != 0;
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
            let _ = (bytes, entry_offset);
            eprintln!("SKIP: shared-borrow callee execution requires a supported host");
        }
    }
}
