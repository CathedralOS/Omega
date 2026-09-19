//! Reference values carried through call results and record fields keep the
//! caller's original referent through publication.

use super::{NativeTarget, borrowed_arguments, native_function};

/// A `&mut` returned by one call feeds a second call's argument; the write
/// lands on the caller's original referent and the entry reads it back.
#[test]
fn returned_reference_relays_to_a_second_call_on_the_caller_referent() {
    let source = "machine relay(value: &mut i32) -> &mut i32 { value }
        machine replace(value: &mut i32) { value = 29; }
        machine exercise(value: &mut i32) -> i32 {
            let held: &mut i32 = relay(value);
            replace(held);
            value
        }";
    assert_i32_referent_publication(source);
}

/// A record-carried `&mut` field feeds a call argument; the write still lands
/// on the caller's original referent.
#[test]
fn record_carried_reference_reaches_the_caller_referent() {
    let source = "data View { body: &mut i32; }
        machine replace(value: &mut i32) { value = 29; }
        machine exercise(value: &mut i32) -> i32 {
            let held: View = View { body: value };
            replace(held.body);
            value
        }";
    assert_i32_referent_publication(source);
}

fn assert_i32_referent_publication(source: &str) {
    for target in borrowed_arguments::hosted_targets() {
        let (bytes, entry) = borrowed_arguments::published_for(source, "exercise", target);
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
            extern int32_t omega_entry(int32_t *value);
            int main(void) {
                struct { uint64_t before; int32_t value; uint64_t after; } frame;
                memset(&frame, 0xa5, sizeof frame);
                frame.value = 5;
                __typeof__(frame) expected = frame;
                expected.value = 29;
                int32_t returned = omega_entry(&frame.value);
                return returned != 29 || memcmp(&expected, &frame, sizeof frame) != 0;
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
            eprintln!("SKIP: reference-carrier execution requires a supported Linux or macOS host");
        }
    }
}
