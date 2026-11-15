//! An owned record sequence such as `[Ticket { number: 11u32 }]` keeps its exact elements when it
//! is held in a local, returned from the machine that built it and passed by value to another
//! machine. Native code backs the sequence with read-only image storage, so its descriptor stays
//! valid after the building machine's frame is gone; the Psi interpreter and the native program
//! must agree on the result. A sequence with a runtime element has no read-only bytes, so native
//! lowering refuses it instead of backing it with a frame that ends at return.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn fixture(tier: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(tier)
        .join("ownership")
        .join(name)
}

fn run(project: &Path, engine: &[&str]) -> (Output, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("run")
        .args(engine)
        .arg("main.omg")
        .current_dir(project)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    (output, report)
}

#[test]
fn an_owned_record_sequence_survives_return_and_by_value_transfer_natively() {
    let project = fixture("run", "owned_record_sequence");
    let expected = std::fs::read_to_string(project.join("expected_native_exit.txt")).unwrap();
    let (output, report) = run(&project, &["--both"]);
    assert!(output.status.success(), "{report}");
    let code = expected.trim();
    for receipt in [
        format!("native exit {code}"),
        format!("interp exit: {code}"),
    ] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn an_owned_record_sequence_with_a_runtime_element_is_refused_natively() {
    let project = fixture("fail", "runtime_record_sequence");
    let (output, report) = run(&project, &["--engine", "native"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("native artifact target lowering failed: UnsupportedControlFlow"),
        "{report}"
    );
}
