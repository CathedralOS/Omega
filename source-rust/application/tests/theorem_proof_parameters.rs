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
    // The Build normally resolves its std dependency beside the checkout.
    // Keep that source when the twin moves into its private working directory.
    let build = std::fs::read_to_string(fixture.join("build.omg")).unwrap();
    let std_location = support::repository_root()
        .join("source/library/std")
        .to_string_lossy()
        .replace('\\', "/");
    let location = "../../../../../../../../../../../source/library/std";
    assert!(build.contains(location));
    std::fs::write(
        scratch.path().join("build.omg"),
        build.replace(location, &std_location),
    )
    .unwrap();
    std::fs::write(scratch.path().join("empty.stdin"), []).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "run",
            "--both",
            "--keep",
            "--input",
            "empty.stdin",
            "--outputs",
            "observations",
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
    if !wrong_proof {
        assert!(output.status.success(), "{report}");
        let observations = scratch.path().join("observations");
        let table = std::fs::read_to_string(observations.join("results.tsv")).unwrap();
        let rows: Vec<_> = table.lines().collect();
        assert_eq!(rows.len(), 2, "{table}");
        let columns: Vec<_> = rows[1].split('\t').collect();
        assert_eq!(columns.len(), 4, "{table}");
        assert_eq!(columns[0], "empty", "{table}");
        assert_eq!(columns[1], "0", "native must exit zero: {table}");
        assert_eq!(columns[3], "0", "Psi must exit zero: {table}");
        assert_eq!(columns[2], "agrees", "{table}");
        for stream in ["stdout", "stderr"] {
            let native = std::fs::read(observations.join(format!("empty.{stream}"))).unwrap();
            let psi = std::fs::read(observations.join(format!("empty.terminal.{stream}"))).unwrap();
            assert!(
                native.is_empty(),
                "native {stream} must be empty: {native:?}"
            );
            assert!(psi.is_empty(), "Psi {stream} must be empty: {psi:?}");
            assert_eq!(native, psi, "{stream}");
        }
        eprintln!("{table}both engines produced the expected empty stdout and stderr");
    }
    (output, report)
}

#[test]
fn an_explicit_named_theorem_runs_without_a_callback() {
    let (output, report) = invoke(false);
    assert!(output.status.success(), "{report}");
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
