//! Proof arguments preserve the supplied implication without executing its source.

#[path = "support/observations.rs"]
mod observations;
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;

fn project(wrong_proof: bool) -> TemporaryDirectory {
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
    scratch
}

const INPUTS: &[&str] = &[
    "--build-input",
    "build.omg",
    "--build-input",
    "main.omg",
    "build.omg",
];

#[test]
fn an_explicit_named_theorem_runs_without_a_callback() {
    observations::assert_silent_return(|| project(false), INPUTS);
}

#[test]
fn a_narrower_theorem_is_refused_before_execution() {
    let (output, report) = observations::run(&project(true), &["--check"], INPUTS);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("explicit proof argument does not establish the complete aliased contract"),
        "{report}",
    );
    assert!(!report.contains("native exit 0"), "{report}");
}
