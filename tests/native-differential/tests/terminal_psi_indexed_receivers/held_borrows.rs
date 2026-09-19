//! `let`-bound borrows are ordinary place aliases: stores through them write
//! the captured referent, calls forward the erased loan's authority, and a
//! live subloan does not block the parent's later receiver calls.

use super::{NativeTarget, native_function, primitive_stores};

fn published_entry(source: &str, target: NativeTarget) -> (Vec<u8>, usize) {
    primitive_stores::published_text(source, target)
}

/// A borrow bound after an earlier call still resolves to its captured
/// element; both receiver stores land on the original caller array in order.
#[test]
fn mid_body_held_borrow_replaces_its_captured_element() {
    let source = "data Record [copy] { before: u8; value: u16; after: u8; }
        machine Record::replace(&write self, value: u16) { self.value = value; }
        machine forward(records: &write [Record; 2], value: u16) {
            records[0].replace(value);
            let held: &write Record = &write records[1];
            held.replace(value);
        }";
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (bytes, entry) = published_entry(source, target);
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
            typedef struct { uint8_t before; uint16_t value; uint8_t after; } Record;
            extern void omega_entry(uint16_t value, Record *records);
            int main(void) {
                struct { uint64_t before; Record records[2]; uint64_t after; } frame;
                memset(&frame, 0xa5, sizeof frame);
                unsigned char expected[sizeof frame];
                frame.records[0].value = 0x1234;
                frame.records[1].value = 0x1234;
                memcpy(expected, &frame, sizeof frame);
                frame.records[0].value = 0;
                frame.records[1].value = 0;
                omega_entry(0x1234, frame.records);
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
            eprintln!("SKIP: mid-body alias observation requires a Linux or macOS host");
        }
    }
}

/// A `let`-bound array borrow indexes its store straight into the caller's
/// elements.
#[test]
fn held_array_borrow_indexes_its_store_into_caller_elements() {
    let source = "machine forward(values: &write [u16; 4]) {
            let held: &write [u16; 4] = &write values;
            held[2] = 17;
        }";
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (bytes, entry) = published_entry(source, target);
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
            extern void omega_entry(uint16_t *values);
            int main(void) {
                struct { uint64_t before; uint16_t values[4]; uint64_t after; } frame;
                memset(&frame, 0xa5, sizeof frame);
                unsigned char expected[sizeof frame];
                frame.values[2] = 17;
                memcpy(expected, &frame, sizeof frame);
                frame.values[2] = 0x5a5a;
                omega_entry(frame.values);
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
            eprintln!("SKIP: held array store observation requires a Linux or macOS host");
        }
    }
}

/// A `let`-bound scalar borrow stores a whole primitive through its captured
/// referent for both `&write` and `&mut` carriers.
#[test]
fn held_scalar_borrow_stores_through_its_captured_referent() {
    for (_access, source) in [
        (
            "write",
            "machine forward(value: &mut u64) {
                let held: &write u64 = &write value;
                held = 17;
            }",
        ),
        (
            "mut",
            "machine forward(root: &mut u64) {
                let held: &mut u64 = &mut root;
                held = 17;
            }",
        ),
    ] {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let (bytes, entry) = published_entry(source, target);
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
                extern void omega_entry(uint64_t *root);
                int main(void) {
                    struct { uint64_t before; uint64_t root; uint64_t after; } frame;
                    memset(&frame, 0xa5, sizeof frame);
                    unsigned char expected[sizeof frame];
                    frame.root = 17;
                    memcpy(expected, &frame, sizeof frame);
                    frame.root = 0;
                    omega_entry(&frame.root);
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
                let _ = (bytes, entry, _access);
                eprintln!("SKIP: held scalar store observation requires a Linux or macOS host");
            }
        }
    }
}

/// A held borrow forwards its own exclusive authority as a call argument —
/// bare for `&mut`, reborrowed for `&write` and `&mut` alike — and the callee
/// store still lands on the original referent.
#[test]
fn held_borrows_forward_exclusive_authority_as_call_arguments() {
    for (_name, source) in [
        (
            "write reborrow",
            "machine stamp(slot: &write u64, value: u64) { slot = value; }
            machine forward(root: &mut u64, value: u64) {
                let held: &write u64 = &write root;
                stamp(&write held, value);
            }",
        ),
        (
            "bare mut carrier",
            "machine stamp(slot: &mut u64, value: u64) { slot = value; }
            machine forward(root: &mut u64, value: u64) {
                let held: &mut u64 = &mut root;
                stamp(held, value);
            }",
        ),
        (
            "mut reborrow",
            "machine stamp(slot: &mut u64, value: u64) { slot = value; }
            machine forward(root: &mut u64, value: u64) {
                let held: &mut u64 = &mut root;
                stamp(&mut held, value);
            }",
        ),
    ] {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let (bytes, entry) = published_entry(source, target);
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
                extern void omega_entry(uint64_t value, uint64_t *root);
                int main(void) {
                    struct { uint64_t before; uint64_t root; uint64_t after; } frame;
                    memset(&frame, 0xa5, sizeof frame);
                    unsigned char expected[sizeof frame];
                    frame.root = 0x0123456789abcdefull;
                    memcpy(expected, &frame, sizeof frame);
                    frame.root = 0x5a5a5a5a5a5a5a5aull;
                    omega_entry(0x0123456789abcdefull, &frame.root);
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
                let _ = (bytes, entry, _name);
                eprintln!("SKIP: held borrow argument observation requires a Linux or macOS host");
            }
        }
    }
}

/// A `&write` element subloan stays live across a later `&mut` receiver call
/// on the parent; each store lands on its own original element.
#[test]
fn live_write_subloan_does_not_block_a_parent_mut_receiver_call() {
    let source = "data Record [copy] { before: u8; value: u16; after: u8; }
        machine Record::replace(&write self, value: u16) { self.value = value; }
        machine Record::bump(&mut self) { self.value = self.value; }
        machine forward(records: &mut [Record; 2], value: u16) {
            let held: &write Record = &write records[1];
            held.replace(value);
            records[0].bump();
        }";
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (bytes, entry) = published_entry(source, target);
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
            typedef struct { uint8_t before; uint16_t value; uint8_t after; } Record;
            extern void omega_entry(uint16_t value, Record *records);
            int main(void) {
                struct { uint64_t before; Record records[2]; uint64_t after; } frame;
                memset(&frame, 0xa5, sizeof frame);
                frame.records[0].value = 0x2468;
                unsigned char expected[sizeof frame];
                frame.records[1].value = 0x1357;
                memcpy(expected, &frame, sizeof frame);
                frame.records[1].value = 0xffff;
                omega_entry(0x1357, frame.records);
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
            eprintln!("SKIP: live subloan observation requires a Linux or macOS host");
        }
    }
}
