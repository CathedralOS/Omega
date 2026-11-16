//! `--validator` and `--every-validator` run independent stage validators
//! inside one compilation and report a row for each; default compilation runs
//! none (wiki/spec/build/command_line.md#independent-stage-validators).
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CASE_FIXTURE: &str = "run/calls/indexed_case_discriminator";
const NAME: &str = "TargetOperationCorrespondence";

// Each test compiles its own copy so concurrent tests never share a build directory.
fn project(test: &str, fixture: &str) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(fixture);
    let copy = std::env::temp_dir().join(format!(
        "omega-stage-validators-{test}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&copy);
    std::fs::create_dir_all(&copy).unwrap();
    for file in ["main.omg", "build.omg"] {
        std::fs::copy(source.join(file), copy.join(file)).unwrap();
    }
    copy
}

fn compile(test: &str, fixture: &str, arguments: &[&str]) -> (bool, String, String) {
    let directory = project(test, fixture);
    let report = directory.join("report.txt");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .arg("--report-file")
        .arg(&report)
        .arg("main.omg")
        .current_dir(&directory)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target");
    let console = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let report = std::fs::read_to_string(report).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&directory);
    (output.status.success(), console, report)
}

fn row(result: &str) -> String {
    let host = target::TargetProfile::from_omega_target_name(None)
        .unwrap()
        .target_name();
    format!("stage validator {NAME} (target {host}, stage target operations): {result}")
}

#[test]
fn a_named_validator_runs_on_the_compiled_target_operations_and_passes() {
    let (success, console, report) =
        compile("named", CASE_FIXTURE, &["--timings", "--validator", NAME]);
    assert!(success, "{console}");
    assert!(console.contains("validator rows"), "{console}");
    assert!(
        console.lines().any(|line| line == row("passed")),
        "{console}"
    );
    assert!(report.lines().any(|line| line == row("passed")), "{report}");
}

#[test]
fn a_fixed_frame_request_retains_and_audits_the_original_allocation() {
    let name = "FixedFrameCorrespondence";
    let (success, console, report) = compile(
        "fixed-frame",
        "run/ownership/borrowed_root_replacement",
        &["--validator", name],
    );
    assert!(success, "{console}");
    let host = target::TargetProfile::from_omega_target_name(None)
        .unwrap()
        .target_name();
    let expected =
        format!("stage validator {name} (target {host}, stage fixed-frame machine code): passed");
    assert!(report.lines().any(|line| line == expected), "{report}");
}

#[test]
fn byte_view_home_retirement_passes_the_named_validator() {
    let (success, console, report) = compile(
        "view-retirement",
        "run/views/byte_view_edge_retirement",
        &["--validator", NAME],
    );
    assert!(success, "{console}");
    assert!(report.lines().any(|line| line == row("passed")), "{report}");
}

#[test]
fn every_validator_reports_what_ran_and_what_was_inapplicable() {
    let (success, console, report) = compile("every", CASE_FIXTURE, &["--every-validator"]);
    assert!(success, "{console}");
    assert!(report.lines().any(|line| line == row("passed")), "{report}");
    // The enumerated set names the compiler and the roster it came from.
    assert!(
        report
            .lines()
            .any(|line| line.starts_with("stage validator roster (omega ")
                && line.ends_with(&format!(
                    "): {NAME}, FixedFrameCorrespondence, SelectedInstructionRewriteReplay"
                ))),
        "{report}"
    );
    // This program selects no selected-instruction rewrite rule, so the
    // replay validator has no run to check and says so instead of passing.
    assert!(
        report.lines().any(|line| line.starts_with(
            "stage validator SelectedInstructionRewriteReplay (target "
        ) && line.ends_with(
            "stage selected-instruction rewrites): inapplicable: no selected-instruction rewrite rule ran"
        )),
        "{report}"
    );
    let (success, console, report) = compile(
        "every-check",
        CASE_FIXTURE,
        &["--check", "--every-validator"],
    );
    assert!(success, "{console}");
    let inapplicable = row("inapplicable: --check produces no native target operations");
    assert!(report.lines().any(|line| line == inapplicable), "{report}");
    assert!(!report.contains("passed"), "{report}");
}

#[test]
fn default_compilation_runs_no_validator() {
    // The timing list names every stage that ran; a validator run adds a
    // `validator rows` step, which default compilation never has.
    let (success, console, report) = compile("default", CASE_FIXTURE, &["--timings"]);
    assert!(success, "{console}");
    assert!(!console.contains("stage validator"), "{console}");
    assert!(!console.contains("validator rows"), "{console}");
    assert!(!report.contains("stage validator"), "{report}");
}

#[test]
fn unknown_and_unproduced_validators_are_invocation_errors() {
    let (success, console, _) =
        compile("unknown", CASE_FIXTURE, &["--validator", "NoSuchValidator"]);
    assert!(!success, "{console}");
    assert!(
        console.contains("unknown stage validator `NoSuchValidator`"),
        "{console}"
    );
    let (success, console, _) = compile(
        "named-check",
        CASE_FIXTURE,
        &["--check", "--every-validator", "--validator", NAME],
    );
    assert!(!success, "{console}");
    assert!(
        console.contains(&format!(
            "stage validator `{NAME}` cannot run in this compilation: --check produces no native target operations"
        )),
        "{console}"
    );
}
