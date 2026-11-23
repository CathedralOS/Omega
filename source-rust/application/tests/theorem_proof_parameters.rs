//! Proof arguments preserve the supplied implication without executing its source.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn invoke(wrong_proof: bool) -> (Output, String) {
    let fixture = support::repository_root().join(
        "source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/proofs/positive/named_implication",
    );
    let scratch = TemporaryDirectory::create("theorem-proof-parameters", false).unwrap();
    let original = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    let source = if wrong_proof {
        let changed = original.replace(
            "use_step::<nonzero_is_positive>(7u8)",
            "use_step::<only_seven>(7u8)",
        );
        assert_ne!(changed, original);
        changed
    } else {
        original
    };
    std::fs::write(scratch.path().join("main.omg"), source).unwrap();
    std::fs::copy(fixture.join("build.omg"), scratch.path().join("build.omg")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "run",
            "--both",
            "--keep",
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "build.omg",
        ])
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("run the original caller with its explicit Build inputs");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    eprintln!("{report}");
    (output, report)
}

#[test]
fn an_explicit_named_theorem_runs_without_a_callback() {
    let (output, report) = invoke(false);
    assert!(output.status.success(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_narrower_theorem_is_refused_before_execution() {
    let (output, report) = invoke(true);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("explicit proof argument does not establish the complete aliased contract"),
        "{report}",
    );
    assert!(!report.contains("native exit 0"), "{report}");
}
