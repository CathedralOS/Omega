//! Checking for one target still checks every target's Build row, because "an
//! explicit `--target` selection narrows realization only and never narrows
//! checking" (build/configuration.md, "Multi-target compilation").
//!
//! - Build binds `launch`, whose precondition is `false`, for the Linux and
//!   macOS targets and a provable entry for Windows. Checking for Windows alone
//!   must refuse the program.
//! - Build selects, for Windows, a provider that calls back into its own
//!   requirement. Checking for Linux alone must refuse the program.
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

const FIXTURE: &str = "source-rust/subsystems/compiler/tests/fixtures/build/fail/program_entry_unestablished_requires";

/// `omega --check --target <target>` over a `build.omg` and `main.omg`.
/// Returns whether it succeeded and what it printed.
fn check(target: &str, build: &str, main: &str) -> (bool, String) {
    let scratch = TemporaryDirectory::create("entry-rows-every-target", false).unwrap();
    std::fs::write(scratch.path().join("main.omg"), main).unwrap();
    std::fs::write(scratch.path().join("build.omg"), build).unwrap();
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

/// The fixture's `build.omg` rewritten by `edit`, and its `main.omg`.
fn check_fixture(target: &str, edit: impl Fn(String) -> String) -> (bool, String) {
    let fixture = support::repository_root().join(FIXTURE);
    let build = std::fs::read_to_string(fixture.join("build.omg")).unwrap();
    let main = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    check(target, &edit(build), &main)
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
        let (success, text) = check_fixture(target, |build| build);
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
    let (success, text) = check_fixture("windows_x86_64", |build| {
        build.replace(", launch);", ", other);")
    });
    assert!(success, "{text}");
}

const CYCLE_BUILD: &str = r#"machine build(builder: @mutable Build) {
    Build::application(builder, "row_provider_cycle");
    Build::bind(builder, windows_x86_64\ProgramEntry, main);
    Build::bind(builder, linux_x86_64\ProgramEntry, main);
    Build::select_provider::<Source, WindowsSource>(builder, windows_x86_64);
    Build::select_provider::<Source, LinuxSource>(builder, linux_x86_64);
}
"#;

/// `WindowsSource` returns `windows_body`; the twins differ only there.
fn cycle_main(windows_body: &str) -> String {
    format!(
        "boundary trait Source {{
    machine source(input: u64) -> u64 reaches none ensures result == 7 crashes none;
}}
WindowsSource satisfies Source {{
    machine source(input: u64) -> u64 reaches none ensures result == 7 {{ {windows_body} }}
}}
LinuxSource satisfies Source {{
    machine source(input: u64) -> u64 reaches none ensures result == 7 {{ 7 }}
}}
machine forwarded(input: u64) -> u64 ensures result == 7 {{ Source::source(input) }}
machine main() crashes Trap {{
    let value: u64 = forwarded(3);
    transition value == 7 {{ true -> done(), _ -> bad() }}
    machine done() {{}}
    machine bad() {{ crash Trap; }}
}}
"
    )
}

#[test]
fn a_provider_cycle_in_another_row_is_refused() {
    let main = cycle_main("forwarded(input)");
    for target in ["linux_x86_64", "windows_x86_64"] {
        let (success, text) = check(target, CYCLE_BUILD, &main);
        assert!(
            !success,
            "{target}: the Windows row selects a cycle\n{text}"
        );
        assert!(
            text.contains("selected providers realize a cyclic synchronous `invokes` graph"),
            "{target}: {text}"
        );
    }
    let (_, text) = check("linux_x86_64", CYCLE_BUILD, &main);
    assert!(text.contains("the `windows_x86_64` Build row:"), "{text}");
}

#[test]
fn rows_whose_providers_return_check() {
    let (success, text) = check("linux_x86_64", CYCLE_BUILD, &cycle_main("7"));
    assert!(success, "{text}");
}
