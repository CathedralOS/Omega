//! Run the same caller-owned activations from source, saved Psi and the native executable.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/declared_exit.rs"]
mod declared_exit;

use declared_exit::project;
use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/calls/run/caller_owned_generators")
}

#[test]
fn the_physical_linux_process_matches_the_interpreted_producers() {
    declared_exit::assert_declared_exit(&fixture(), "build.omg");
}

#[test]
fn interleaved_producers_keep_their_publications_locals_and_cleanup() {
    let fixture = fixture();
    let expected: i32 = std::fs::read_to_string(fixture.join("expected_native_exit.txt"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let scratch = TemporaryDirectory::create("caller-owned-generators-replay", false).unwrap();
    let source_run = scratch.path().join("source");
    assert_psi_exit(&fixture, Path::new("build.omg"), &source_run, expected);

    let artifact = scratch.path().join("generators.psi");
    let output = project::omega(
        &fixture,
        &[
            "--emit-psi",
            artifact.to_str().unwrap(),
            "--target",
            "linux_x86_64",
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "build.omg",
        ],
    );
    assert!(output.status.success(), "{output:?}");

    let replay = scratch.path().join("replay");
    assert_psi_exit(scratch.path(), &artifact, &replay, expected);
    for stream in ["stdout", "stderr"] {
        assert_eq!(
            std::fs::read(source_run.join(stream)).unwrap(),
            std::fs::read(replay.join(stream)).unwrap(),
            "{stream}"
        );
    }
}

fn assert_psi_exit(directory: &Path, program: &Path, outputs: &Path, expected: i32) {
    let output = project::omega(
        directory,
        &[
            "run",
            "--outputs",
            outputs.to_str().unwrap(),
            program.to_str().unwrap(),
        ],
    );
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        std::fs::read_to_string(outputs.join("termination.tsv")).unwrap(),
        format!("psi\texit\t{expected}\n")
    );
}

#[test]
fn an_owner_or_answer_permit_cannot_be_used_twice() {
    let source = include_str!("../../../tests/fixtures/calls/run/caller_owned_generators/main.omg");
    let original = "answer(move crossed.owner, move left_item.permit, crossed.answer)";
    assert_eq!(source.matches(original).count(), 1);
    for repeated in [
        "answer(move right_parked, move left_item.permit, crossed.answer)",
        "answer(move crossed.owner, move right_item.permit, crossed.answer)",
    ] {
        let invalid = source.replace(original, repeated);
        let (accepted, report) = project::check(
            "duplicated-generator-custody",
            &[("main.omg", &invalid)],
            &["--check", "main.omg"],
        );
        assert!(!accepted, "{report}");
        assert!(
            report.contains("owned storage is used after its value was transferred or consumed"),
            "{report}"
        );
    }
}
