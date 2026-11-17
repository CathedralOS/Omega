//! `Clock::sleep` holds the caller's worker for the whole wait, so the standard
//! library declares it `blocks` and every caller must write `block` at the
//! call (wiki/spec/language/effects.md, call-site acknowledgements). Before
//! the declaration, std published the wait as never blocking and the checker
//! accepted an unmarked call.
//!
//! The twins differ only in that marker. The positive still stops later, at a
//! separate failure every std `Clock` program shares today, so it pins only
//! that the acknowledgement check accepts the marked call.

use std::path::{Path, PathBuf};
use std::process::Command;

const MISSING_BLOCK: &str = "call may block and requires a block acknowledgement";

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(relative)
}

fn omega_check(directory: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--check", "--verbose", "main.omg"])
        .current_dir(directory)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn an_unmarked_clock_sleep_is_refused() {
    let stderr = omega_check(&fixture("fail/effects/clock_sleep_unacknowledged"));
    assert!(
        stderr.contains(&format!("{MISSING_BLOCK} in machine `entry_start`")),
        "{stderr}"
    );
}

#[test]
fn a_marked_clock_sleep_passes_the_acknowledgement_check() {
    let stderr = omega_check(&fixture("pass/effects/clock_sleep_acknowledged"));
    assert!(!stderr.contains(MISSING_BLOCK), "{stderr}");
}
