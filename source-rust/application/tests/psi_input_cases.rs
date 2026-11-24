//! A single target-bound Psi product is instantiated independently for each input.
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::{
    fs,
    path::Path,
    process::{Command, Output, Stdio},
};

fn batch(project: &Path, options: &[&str], directory: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(project)
        .args(["run", "--engine", "psi", "--target", "linux_x86_64"])
        .args(options)
        .arg("--outputs")
        .arg(directory)
        .arg("build.omg")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn project() -> TemporaryDirectory {
    let scratch = TemporaryDirectory::create("psi-input-cases", false).unwrap();
    let fixture = support::repository_root()
        .join("source-rust/application/tests/fixtures/cli/run/psi_input_cases");
    for file in ["build.omg", "main.omg"] {
        fs::copy(fixture.join(file), scratch.path().join(file)).unwrap();
    }
    let inputs = scratch.path().join("inputs");
    fs::create_dir(&inputs).unwrap();
    for (name, bytes) in [
        ("z.stdin", &b"Z\0"[..]),
        ("short.stdin", b"X"),
        ("a.stdin", b"\xffB"),
    ] {
        fs::write(inputs.join(name), bytes).unwrap();
    }
    scratch
}

#[test]
fn directory_cases_keep_finite_binary_streams_and_guest_termination_in_name_order() {
    let scratch = project();
    let directory = scratch.path().join("observations");
    let execution = batch(scratch.path(), &["--inputs", "inputs"], &directory);
    assert!(
        execution.status.success(),
        "{}",
        String::from_utf8_lossy(&execution.stderr)
    );
    let table = fs::read_to_string(directory.join("results.tsv")).unwrap();
    assert_eq!(
        table,
        "case\ttermination\tdetail\na\treturn\t\nshort\tcrash\tTrap\nz\treturn\t\n"
    );
    for (name, bytes) in [("a", &b"\xffB"[..]), ("z", b"Z\0")] {
        assert_eq!(
            fs::read(directory.join(format!("{name}.stdout"))).unwrap(),
            bytes
        );
        assert_eq!(
            fs::read(directory.join(format!("{name}.stderr"))).unwrap(),
            b"!"
        );
        assert!(
            fs::read_to_string(directory.join(format!("{name}.psi.work.tsv")))
                .unwrap()
                .contains("work_used\t")
        );
    }
}

#[test]
fn explicit_cases_record_each_exhausted_allowance_in_argument_order() {
    let scratch = project();
    let directory = scratch.path().join("limited");
    let execution = batch(
        scratch.path(),
        &[
            "--psi-work-budget",
            "1",
            "--input",
            "inputs/z.stdin",
            "--input",
            "inputs/a.stdin",
        ],
        &directory,
    );
    assert_eq!(
        execution.status.code(),
        Some(200),
        "{}",
        String::from_utf8_lossy(&execution.stderr)
    );
    let table = fs::read_to_string(directory.join("results.tsv")).unwrap();
    let rows: Vec<_> = table.lines().skip(1).collect();
    assert_eq!(rows.len(), 2, "{table}");
    for (row, name) in rows.iter().zip(["z", "a"]) {
        assert!(row.starts_with(&format!("{name}\tincomplete\t")), "{table}");
        assert!(
            fs::read_to_string(directory.join(format!("{name}.psi.work.tsv")))
                .unwrap()
                .starts_with("work_budget\t1\n")
        );
    }
}
