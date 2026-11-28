//! Both producer activations run from the same authored program and library on every route.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/declared_exit.rs"]
mod declared_exit;

use declared_exit::project;
use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;

#[test]
fn interleaved_producers_keep_their_publications_locals_and_cleanup() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/calls/run/caller_owned_generators");
    declared_exit::assert_declared_exit(&fixture, "build.omg");

    let scratch = TemporaryDirectory::create("caller-owned-generators-replay", false).unwrap();
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

    let observations = scratch.path().join("observations");
    let output = project::omega(
        scratch.path(),
        &[
            "run",
            "--outputs",
            observations.to_str().unwrap(),
            artifact.to_str().unwrap(),
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let expected = std::fs::read_to_string(fixture.join("expected_native_exit.txt")).unwrap();
    assert_eq!(
        std::fs::read_to_string(observations.join("termination.tsv")).unwrap(),
        format!("psi\texit\t{}\n", expected.trim())
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
