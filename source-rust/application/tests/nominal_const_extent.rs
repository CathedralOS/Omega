//! Nominal extents retain each constant’s declared integer type.

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn invoke(tier: &str, arguments: &[&str]) -> (Output, String) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(tier)
        .join("generics/nominal_const_extent");
    let scratch = TemporaryDirectory::create("nominal-const-extent", false).unwrap();
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
fn nominal_extents_survive_generic_headers_in_both_engines() {
    let (output, report) = invoke("run", &["run", "--both"]);
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_const_argument_cannot_change_its_declared_width() {
    let (output, report) = invoke("fail", &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("record const argument does not inhabit its declared carrier"),
        "{report}"
    );
}
