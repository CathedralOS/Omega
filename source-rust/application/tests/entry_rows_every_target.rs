//! Checking for one target still proves every target's entry row. Build binds
//! `launch`, whose precondition is `false`, for the Linux and macOS targets and
//! a provable entry for Windows. Checking for Windows alone must refuse the
//! program, because "an explicit `--target` selection narrows realization only
//! and never narrows checking" (build/configuration.md, "Multi-target
//! compilation").
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

const FIXTURE: &str = "source-rust/subsystems/compiler/tests/fixtures/build/fail/program_entry_unestablished_requires";

/// `omega --check --target <target>` over the fixture, with `build.omg`
/// rewritten by `edit`. Returns whether it succeeded and what it printed.
fn check(target: &str, edit: impl Fn(String) -> String) -> (bool, String) {
    let fixture = support::repository_root().join(FIXTURE);
    let scratch = TemporaryDirectory::create("entry-rows-every-target", false).unwrap();
    std::fs::copy(fixture.join("main.omg"), scratch.path().join("main.omg")).unwrap();
    let build = std::fs::read_to_string(fixture.join("build.omg")).unwrap();
    std::fs::write(scratch.path().join("build.omg"), edit(build)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--check", "--target", target])
        .args([
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "build.omg",
        ])
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), text)
}

#[test]
fn a_row_for_another_target_is_checked() {
    let expected = std::fs::read_to_string(
        support::repository_root()
            .join(FIXTURE)
            .join("expected.txt"),
    )
    .unwrap();
    let expected = expected.lines().next().unwrap();
    for target in ["windows_x86_64", "linux_x86_64"] {
        let (success, text) = check(target, |build| build);
        assert!(
            !success,
            "{target}: the Linux and macOS rows bind `launch`\n{text}"
        );
        assert!(text.contains(expected), "{target}: {text}");
    }
}

#[test]
fn every_row_with_a_provable_entry_checks() {
    // The twin binds the provable entry for every target.
    let (success, text) = check("windows_x86_64", |build| {
        build.replace(", launch);", ", other);")
    });
    assert!(success, "{text}");
}
