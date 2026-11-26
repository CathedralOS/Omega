//! Each engine must satisfy the authored expectation even when its peer agrees.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

#[path = "support/project.rs"]
mod project;

const BUILD: &str = include_str!("fixtures/build/run/registered_streams/build.omg");
const PROGRAM: &str = include_str!("fixtures/build/run/registered_streams/main.omg");

fn compare(configuration: &str) -> (bool, String) {
    project::check(
        "registered-streams",
        &[("build.omg", configuration), ("main.omg", PROGRAM)],
        &["build", "--compare-tests", "build.omg"],
    )
}

#[test]
fn every_test_and_engine_replays_an_independent_copy_of_finite_stdin() {
    let (success, output) = compare(BUILD);
    assert!(success, "{output}");
    for (test, group) in [
        ("first_echo", "echo"),
        ("second_echo", "echo"),
        ("default_input_is_empty", "empty_input"),
    ] {
        let row = output
            .lines()
            .find(|row| row.contains(&format!("test `{test}` in group `Checks::{group}`")))
            .unwrap_or_else(|| panic!("{output}"));
        assert!(
            row.contains("passed on Psi interpreter, native")
                && row.contains("native/Psi comparison agrees"),
            "{row}"
        );
    }
}

#[test]
fn matching_wrong_output_fails_each_engine_and_keeps_the_comparison() {
    for (config, mismatch) in [
        (
            BUILD.replace("\"99\"", "\"42\""),
            "stdout byte 0: expected 0x34, observed 0x39",
        ),
        (
            BUILD.replace("\"!\"", "\"\""),
            "stderr byte 0: expected end of stream, observed 0x21",
        ),
    ] {
        let (success, output) = compare(&config);
        assert!(!success, "{output}");
        for test in ["first_echo", "second_echo"] {
            let row = output
                .lines()
                .find(|row| row.contains(&format!("test `{test}` in group `Checks::echo`")))
                .unwrap_or_else(|| panic!("{output}"));
            assert!(
                row.contains(&format!("failed on the Psi interpreter engine: {mismatch}"))
                    && row.contains(&format!("failed on the native engine: {mismatch}"))
                    && row.contains("native/Psi comparison agrees"),
                "{row}"
            );
        }
    }
}
