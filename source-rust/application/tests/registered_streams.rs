//! Each engine must satisfy the authored expectation even when its peer agrees.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

const BUILD: &str = include_str!("fixtures/build/run/registered_streams/build.omg");
const PROGRAM: &str = include_str!("fixtures/build/run/registered_streams/main.omg");

fn compare(configuration: &str) -> (bool, String) {
    let root = TemporaryDirectory::create("registered-streams", false).unwrap();
    std::fs::write(root.path().join("build.omg"), configuration).unwrap();
    std::fs::write(root.path().join("main.omg"), PROGRAM).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["build", "--compare-tests", "build.omg"])
        .current_dir(root.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        result.status.success(),
        String::from_utf8_lossy(&result.stderr).into_owned(),
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
