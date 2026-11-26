//! Active-case visitation admits only reachable payload callbacks and checks
//! the declared callback interfaces even when the applied sum is empty.
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

fn fixture(tier: &str, name: &str) -> PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/reflection")
        .join(tier)
        .join(name)
}

fn invoke(fixture: &Path, source: &str, args: &[&str]) -> (ExitStatus, String) {
    let scratch = TemporaryDirectory::create("active-case-visitation", false).unwrap();
    std::fs::copy(fixture.join("build.omg"), scratch.path().join("build.omg")).unwrap();
    std::fs::write(scratch.path().join("main.omg"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(args)
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
    (
        output.status,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn run_visitation(name: &str) {
    if target::TargetProfile::host_if_supported().is_none() {
        return;
    }
    let fixture = fixture("run", name);
    let source = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    for (engine, receipt) in [("native", "native exit 0"), ("psi", "Psi exit 0")] {
        let (status, diagnostics) = invoke(&fixture, &source, &["run", "--engine", engine]);
        assert!(status.success(), "{diagnostics}");
        assert!(
            diagnostics.lines().any(|line| line == receipt),
            "{diagnostics}"
        );
    }
}

fn rejects_visitation(name: &str) {
    let fixture = fixture("fail", name);
    let source = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    let (status, diagnostics) = invoke(&fixture, &source, &["--check", "--target", "macos_arm64"]);
    assert!(
        !status.success(),
        "{name}: accepted an incompatible callback"
    );
    let expected = std::fs::read_to_string(fixture.join("expected.txt")).unwrap();
    for line in expected.lines() {
        assert!(diagnostics.contains(line), "{name}: {diagnostics}");
    }
}

#[test]
fn active_case_visitors_report_and_update_fields_in_order() {
    run_visitation("active_case_visitation");
}

#[test]
fn active_case_visitors_omit_only_provably_empty_cases() {
    run_visitation("active_case_uninhabited_payload");
    rejects_visitation("active_case_uninhabited_payload");
}

#[test]
fn active_case_visitors_check_both_callback_contracts() {
    for name in [
        "active_case_callback_precondition",
        "active_case_mutable_callback_type",
    ] {
        rejects_visitation(name);
    }
}

#[test]
fn an_empty_sum_still_requires_the_published_callback_interfaces() {
    let fixture = fixture("run", "active_case_uninhabited_payload");
    let source = std::fs::read_to_string(fixture.join("main.omg"))
        .unwrap()
        .replace(
            "data Outcome<T, E> {",
            "data Outcome<T, E> requires false {",
        );
    for (declaration, invalid) in [
        (
            r"selected: @reflect\CaseInfo) {",
            r"selected: @reflect\CaseInfo) requires calls == 0u64 {",
        ),
        ("requires Kind == u64", "requires calls == 0u64"),
    ] {
        assert!(source.contains(declaration));
        let bad = source.replace(declaration, invalid);
        let (status, diagnostics) = invoke(&fixture, &bad, &["--check", "--target", "macos_arm64"]);
        assert!(!status.success());
        assert!(
            diagnostics.contains(
                "it declares a precondition on its inputs that the binder does not publish"
            ),
            "{diagnostics}"
        );
    }
}
