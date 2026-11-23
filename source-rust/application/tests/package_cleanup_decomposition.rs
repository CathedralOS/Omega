//! A record declared `dropped by finish` can still be taken apart by any
//! other machine of its own package with a complete consuming pattern.
//! Here `release` takes one `Lease` apart and returns its mark, and the
//! other `Lease` dies at the end of `main`, where `finish` disposes of it.
//!
//! The checker used to refuse every such program with "selected declaration
//! lost its exact canonical package owner": it read the decomposing
//! machine's package from its hermetic semantic identity, which is not in
//! the type identity grammar. It now reads the machine's nominal identity.
//! The refusal of a machine from another package is tested beside that
//! check, in `terminal-semantics`'s `structural/nominal_cleanup_tests.rs`.
//!
//! `wiki/spec/language/ownership.md`, "Paths and control flow": a record
//! with a selected cleanup "cannot be partially moved, and only an
//! authorized complete consuming pattern takes it apart".

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

#[test]
fn a_machine_of_the_records_package_takes_it_apart_in_native_code_and_psi() {
    let source = std::fs::read_to_string(support::repository_root().join(
        "source-rust/application/tests/fixtures/ownership/run/\
             package_decomposes_selected_cleanup_record/main.omg",
    ))
    .unwrap();
    let scratch = TemporaryDirectory::create("package-cleanup-decomposition", false).unwrap();
    std::fs::write(scratch.path().join("main.omg"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "main.omg"])
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("the compiler runs on the fixture");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}
