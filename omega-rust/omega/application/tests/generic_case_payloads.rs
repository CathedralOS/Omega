//! Generic cases retain active payloads without inventing contents in empty cases.

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn invoke(tier: &str, arguments: &[&str]) -> (Output, String) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(tier)
        .join("ownership/generic_case_payloads");
    let scratch = TemporaryDirectory::create("generic-case-payloads", false).unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(fixture.join(name), scratch.path().join(name)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .args([
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "main.omg",
        ])
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("the compiler runs with the fixture's explicit inputs");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{report}");
    (output, report)
}

#[test]
fn generic_case_payloads_survive_construction_and_calls_in_both_engines() {
    let (output, report) = invoke("run", &["run", "--both"]);
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn an_empty_generic_result_cannot_discard_its_owned_argument() {
    let source =
        include_str!("../../../../tests/fixtures/fail/ownership/generic_case_payloads/main.omg");
    let declaration = source.find("machine observe<T>").unwrap();
    let result = declaration + source[declaration..].find("{ Outcome::Missing }").unwrap() + 2;
    let expected_span = format!("bytes {}..{}:", result, result + "Outcome::Missing".len());
    let (output, report) = invoke("fail", &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(&expected_span), "{report}");
    assert!(
        report.contains(
            "normal edge cannot implicitly dispose storage without an authorized disposition"
        ),
        "{report}"
    );
}
