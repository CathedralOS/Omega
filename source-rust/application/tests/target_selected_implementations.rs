/*

The Build, not the source, chooses which implementation backs a requirement on
each target (`wiki/spec/build/provider_selection.md`, "Build-owned target
selection"). A program declares ordinary named implementations, none of them
marked with a target, and its Build has one `Build::select_provider` row per
target.

The positive program exercises the host row in the compiled executable and
interprets the row for each hosted target. Its twin drops the
`windows_x86_64` row: the three implementations are still in scope, but the
compiler must refuse the program for that target rather than pick one.

*/

#[path = "support/project.rs"]
mod project;
mod support;

use std::path::{Path, PathBuf};
use std::process::Output;

fn fixture(relative: &str) -> PathBuf {
    support::repository_root().join(relative)
}

fn omega_run(directory: &Path, extra: &[&str]) -> Output {
    project::omega(directory, &[&["run"][..], extra, &["build.omg"]].concat())
}

fn printed(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_interpreted_and_compiled_return(directory: &Path) {
    let interpreted = omega_run(directory, &[]);
    let text = printed(&interpreted);
    assert!(
        interpreted.status.success() && text.lines().any(|line| line == "Psi exit 0"),
        "{text}"
    );
    let compiled = project::build_and_run(directory, &["build.omg"]);
    assert!(compiled.status.success(), "{compiled:?}");
}

#[test]
fn each_target_runs_the_implementation_its_build_row_selects() {
    let directory = fixture("tests/fixtures/providers/run/target_selected_implementation");
    assert_interpreted_and_compiled_return(&directory);
    for target in [
        "windows_x86_64",
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
    ] {
        let output = omega_run(&directory, &["--target", target]);
        let text = printed(&output);
        assert!(output.status.success(), "{target}: {text}");
        assert!(text.contains("Psi exit 0"), "{target}: {text}");
    }
}

#[test]
fn a_target_without_a_build_row_has_no_implementation() {
    let directory = fixture(
        "source-rust/application/tests/fixtures/build/fail/target_selected_implementation_missing_row",
    );
    let output = omega_run(
        &directory,
        // Both requirements share one cause; ask for every diagnostic so the
        // assertions below check both exact missing selections.
        &["--verbose", "--target", "windows_x86_64"],
    );
    let text = printed(&output);
    assert!(!output.status.success(), "{text}");
    for member in ["record", "no_follow_flag"] {
        let refusal = format!(
            "boundary requirement `TargetConstants::{member}` has no provider on target \
             `windows_x86_64`: the Build selects none and the target package declares no default"
        );
        assert!(text.contains(&refusal), "{text}");
    }
}

#[test]
fn applied_services_keep_their_separate_selected_implementations() {
    let directory =
        fixture("source-rust/application/tests/fixtures/providers/run/applied_boundary_providers");
    assert_interpreted_and_compiled_return(&directory);
}

#[test]
fn an_implementation_of_another_service_application_is_refused() {
    let directory =
        fixture("source-rust/application/tests/fixtures/providers/fail/applied_boundary_providers");
    let output = omega_run(&directory, &[]);
    let text = printed(&output);
    assert!(!output.status.success(), "{text}");
    assert!(
        text.contains("provider `First` does not satisfy the selected application of `Service`"),
        "{text}"
    );
}

// A selected implementation is checked in its conformance witness, where its
// nested `machine ok() -> i32 { 0i32 }` is an exact checked call in the same
// unit. Its body-derived `result == 0i32` proves the implementation's
// `ensures result == 0i32` (`wiki/spec/language/state_contracts.md`, "Local
// call summaries"); the twin's `ok()` returns 1 and the `ensures` fails.
#[test]
fn a_selected_implementation_uses_its_nested_machines_results() {
    let positive = "source-rust/application/tests/fixtures/providers/run/nested_result_summary";
    let negative = "source-rust/application/tests/fixtures/providers/fail/nested_result_summary";
    let source =
        |directory: &str| std::fs::read_to_string(fixture(directory).join("main.omg")).unwrap();
    let body = |text: &str| text[text.find("boundary trait").unwrap()..].to_owned();
    assert_eq!(
        body(&source(positive)).replace("            0i32\n", "            1i32\n"),
        body(&source(negative))
    );
    assert_interpreted_and_compiled_return(&fixture(positive));
    let output = omega_run(&fixture(negative), &[]);
    let text = printed(&output);
    assert!(!output.status.success(), "{text}");
    assert!(
        text.contains(
            "cannot prove ensures contract for exit from Evaluator::evaluate at statement 1: result == 0"
        ),
        "{text}"
    );
}
