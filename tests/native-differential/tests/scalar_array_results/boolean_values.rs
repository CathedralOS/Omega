//! Computed Booleans retain their value through scalar and array return paths.

use super::*;

const COMPARISONS: &str =
    include_str!("../../../omega/pass/collections/owned_array_scalar_comparisons/main.omg");

#[test]
fn owned_array_scalar_comparisons_return_scalar_boolean_values() {
    publish_comparison("scalar_comparison", false);
}

#[test]
fn owned_array_scalar_comparisons_return_array_boolean_values() {
    publish_comparison("array_comparison", true);
}

fn publish_comparison(entry: &str, array: bool) {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (image, offset) = publish(COMPARISONS, entry, target);
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        if target == NativeTarget::host() {
            let (result, assertion) = if array {
                (
                    "typedef struct { uint8_t values[2]; } Result;",
                    "result.values[0] != expected || result.values[1] != 1",
                )
            } else {
                ("typedef uint8_t Result;", "result != expected")
            };
            let driver = format!(
                "#include <stdint.h>\n{result}\nextern Result omega_entry(uint16_t value);\nint main(void) {{ const uint16_t inputs[] = {{0, 65534, 65535}}; for (unsigned index = 0; index < 3; ++index) {{ const uint8_t expected = inputs[index] == 65535; const Result result = omega_entry(inputs[index]); if ({assertion}) return 1; }} return 0; }}"
            );
            native_function::assert_c_text(&image.output().final_text_bytes, offset, &driver);
        }
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        {
            let _ = (image, offset, array);
            eprintln!("SKIP: native Boolean value execution requires the Linux/macOS host harness");
        }
    }
}

#[test]
fn boolean_values_compose_with_calls_negation_and_multiple_uses() {
    let source = r"
        machine less(left: u64, right: u64) -> bool { left < right }
        machine forward(left: u64, right: u64) -> bool { less(left, right) }
        machine disturb(left: u64, right: u64) -> bool { left <= right }
        machine use_boolean(value: bool) -> bool { value }
        machine values(left: u64, right: u64) -> [bool; 4] {
            let retained: bool = left < right;
            let other: bool = disturb(right, left);
            let forwarded: bool = use_boolean(retained);
            let checked: bool = forward(left, right);
            [forwarded, !retained, other, checked]
        }
    ";
    let driver = r"#include <stdint.h>
        typedef struct { uint8_t values[4]; } Result;
        extern Result omega_entry(uint64_t left, uint64_t right);
        int main(void) {
            const uint64_t inputs[][2] = {{0,0},{5,5},{0,UINT64_MAX},{UINT64_MAX,0},{7,8},{8,7}};
            for (unsigned index = 0; index < 6; ++index) {
                uint64_t left = inputs[index][0], right = inputs[index][1];
                Result result = omega_entry(left, right);
                if (result.values[0] != (left < right) || result.values[1] != !(left < right)
                    || result.values[2] != (right <= left) || result.values[3] != (left < right)) return 1;
            }
            return 0;
        }";
    publish_boolean_source(source, "values", driver);
}

#[test]
fn signed_boolean_values_preserve_equality_and_order() {
    let source = r"
        machine values(left: i64, right: i64) -> [bool; 4] {
            [left == right, left < right, left <= right, !(left <= right)]
        }
    ";
    let driver = r"#include <stdint.h>
        typedef struct { uint8_t values[4]; } Result;
        extern Result omega_entry(int64_t left, int64_t right);
        int main(void) {
            const int64_t inputs[][2] = {{0,0},{5,5},{INT64_MIN,0},{INT64_MAX,-1},{-1,INT64_MIN},{7,8}};
            for (unsigned index = 0; index < 6; ++index) {
                int64_t left = inputs[index][0], right = inputs[index][1];
                Result result = omega_entry(left, right);
                if (result.values[0] != (left == right) || result.values[1] != (left < right)
                    || result.values[2] != (left <= right) || result.values[3] != !(left <= right)) return 1;
            }
            return 0;
        }";
    publish_boolean_source(source, "values", driver);
}

fn publish_boolean_source(source: &str, entry: &str, driver: &str) {
    publish_boolean_source_with_replay(source, entry, driver, ReplayExpectation::Required);
}
fn publish_boolean_source_with_replay(
    source: &str,
    entry: &str,
    driver: &str,
    replay: ReplayExpectation,
) {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let plan = target_plan(source, entry, target).unwrap();
        let (image, offset) = publish_target_with_replay_expectation(plan, entry, target, replay);
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        if target == NativeTarget::host() {
            native_function::assert_c_text(&image.output().final_text_bytes, offset, driver);
        }
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        {
            let _ = (image, offset, driver);
            eprintln!("SKIP: native Boolean execution requires Linux/macOS host harness");
        }
    }
}

#[test]
fn boolean_literal_equality_returns_retain_exact_parameter_polarity() {
    for literal in ["true", "false"] {
        let source = format!(
            "machine keep(value: bool) -> bool {{ value == {literal} }} machine forward(value: bool) -> bool {{ keep(value) }}"
        );
        let driver = format!(
            "#include <stdint.h>\n#include <stdbool.h>\nextern uint8_t omega_entry(bool value);\nint main(void) {{ for (int value = 0; value < 2; ++value) if (omega_entry(value) != (value == {literal})) return 1; return 0; }}"
        );
        for entry in ["keep", "forward"] {
            publish_boolean_source_with_replay(
                &source,
                entry,
                &driver,
                ReplayExpectation::ScalarRecords,
            );
        }
    }
}

#[test]
fn boolean_equality_values_compose_with_array_materialization() {
    let source = "machine values(left: bool, right: bool) -> [bool; 4] { let equal: bool = left == right; [equal, !equal, left == true, left == false] }";
    let driver = r"#include <stdint.h>
        #include <stdbool.h>
        typedef struct { uint8_t values[4]; } Result;
        extern Result omega_entry(bool left, bool right);
        int main(void) { for (int left = 0; left < 2; ++left) for (int right = 0; right < 2; ++right) {
            Result result = omega_entry(left, right);
            if (result.values[0] != (left == right) || result.values[1] != (left != right) || result.values[2] != left || result.values[3] != !left) return 1;
        } return 0; }";
    publish_boolean_source(source, "values", driver);
}
