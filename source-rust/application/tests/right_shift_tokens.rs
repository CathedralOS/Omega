//! A right shift is two `>` that touch, and the same two `>` close two nested
//! generic argument lists (wiki/spec/language/expressions.md). The run
//! program uses both and runs in both engines; its twin puts a space between
//! the two `>` of the shift and is refused at the second one.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn fixture(tier: &str) -> PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/numeric")
        .join(tier)
        .join("right_shift_tokens")
}

fn invoke(tier: &str, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("right-shift-tokens", false).unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(fixture(tier).join(name), scratch.path().join(name)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .arg("main.omg")
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("run the right shift program");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output, report)
}

#[test]
fn touching_greater_tokens_shift_and_close_nested_lists_in_both_engines() {
    let (output, report) = invoke("run", &["run", "--both"]);
    assert!(output.status.success(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_space_between_the_two_greater_tokens_is_not_a_shift() {
    let expected = std::fs::read_to_string(fixture("fail").join("expected.txt")).unwrap();
    let (output, report) = invoke("fail", &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(expected.trim()), "{report}");
}
