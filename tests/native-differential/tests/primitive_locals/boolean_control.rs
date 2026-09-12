//! Branches observe the original Boolean local after a borrowed callee writes it.
use super::{produce, publication};

#[test]
fn caller_branches_on_fresh_boolean_read_after_scalar_call() {
    assert_boolean_observation(
        "machine replace(destination: &mut bool, replacement: bool) -> u64 {
            destination = replacement;
            0
        }
        machine observe(initial: bool, replacement: bool) -> u64 {
            let mut scratch: bool = initial;
            let ignored: u64 = replace(&mut scratch, replacement);
            transition scratch { true -> 1 false -> 0 }
        }",
        false,
    );
}

#[test]
fn caller_branches_on_fresh_boolean_read_after_unit_call() {
    assert_boolean_observation(
        "machine replace(destination: &mut bool, replacement: bool) {
            destination = replacement;
        }
        machine observe(initial: bool, replacement: bool) -> u64 {
            let mut scratch: bool = initial;
            replace(&mut scratch, replacement);
            transition scratch { true -> 1 false -> 0 }
        }",
        false,
    );
}

#[test]
fn negated_local_read_preserves_polarity_after_resultless_and_scalar_helpers() {
    // Swapping the arms as well as negating the condition keeps the replacement
    // oracle; all input pairs distinguish a fresh read from the initializer.
    for (result_type, returned_value) in [("", ""), (" -> u64", "0")] {
        for access in ["mut", "write"] {
            let invocation = if result_type.is_empty() {
                format!("replace(&{access} scratch, replacement);")
            } else {
                format!("let ignored: u64 = replace(&{access} scratch, replacement);")
            };
            assert_boolean_observation(
                &format!(
                    "machine replace(destination: &{access} bool, replacement: bool){result_type} {{
                        destination = replacement;
                        {returned_value}
                    }}
                    machine observe(initial: bool, replacement: bool) -> u64 {{
                        let mut scratch: bool = initial;
                        {invocation}
                        transition !scratch {{ true -> 0 false -> 1 }}
                    }}"
                ),
                false,
            );
        }
    }
}

#[test]
fn caller_branches_on_retained_boolean_snapshot_after_scalar_call() {
    assert_boolean_observation(
        "machine replace(destination: &mut bool, replacement: bool) -> u64 {
            destination = replacement;
            0
        }
        machine observe(initial: bool, replacement: bool) -> u64 {
            let mut scratch: bool = initial;
            let snapshot: bool = scratch;
            let ignored: u64 = replace(&mut scratch, replacement);
            transition snapshot { true -> 1 false -> 0 }
        }",
        true,
    );
}

fn assert_boolean_observation(source: &str, expect_initial: bool) {
    let artifact = produce(source, "observe");
    publication::assert_four_targets(&artifact);
    let driver = format!(
        "#define EXPECT_INITIAL {}\n{}",
        u8::from(expect_initial),
        r#"
#include <stdbool.h>
#include <stdint.h>
#include <unistd.h>
extern uint64_t omega_entry(bool initial, bool replacement);
int main(void) {
    alarm(10);
    for (unsigned initial = 0; initial != 2; ++initial) {
        for (unsigned replacement = 0; replacement != 2; ++replacement) {
            unsigned expected = EXPECT_INITIAL ? initial : replacement;
            if (omega_entry(initial != 0, replacement != 0) != expected) return 1;
        }
    }
    return 0;
}
"#,
    );
    publication::assert_host_execution(&artifact, &driver);
}
