//! Indexed borrows retain the actual byte address through a block edge.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn invoke(tier: &str, arguments: &[&str]) -> (Output, String) {
    let fixture = support::repository_root()
        .join("source-rust/application/tests/fixtures/generics")
        .join(tier)
        .join("indexed_borrow_capture");
    let scratch = TemporaryDirectory::create("indexed-borrow-capture", false).unwrap();
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
fn indexed_borrows_keep_their_original_bytes_in_both_engines() {
    let (output, report) = invoke("run", &["run", "--both"]);
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_shared_indexed_borrow_cannot_become_mutable() {
    let (output, report) = invoke("fail", &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("the caller lends only immutable access to this argument and its parameter writes through it"),
        "{report}"
    );
}
