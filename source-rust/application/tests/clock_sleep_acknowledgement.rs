//! `Clock::sleep` holds the caller's worker for the whole wait, so the standard
//! library declares it `blocks` and every caller must write `block` at the
//! call (wiki/spec/language/effects.md, call-site acknowledgements). Before
//! the declaration, system published the wait as never blocking and the checker
//! accepted an unmarked call.
//!
//! The twins differ only in that marker. The positive still stops later, at a
//! separate failure every system `Clock` program shares today, so it pins only
//! that the acknowledgement check accepts the marked call.

#[path = "support/project.rs"]
mod project;
mod support;

use std::path::{Path, PathBuf};

const MISSING_BLOCK: &str = "call may block and requires a block acknowledgement";

fn fixture(relative: &str) -> PathBuf {
    support::repository_root().join(relative)
}

fn omega_check(directory: &Path) -> String {
    let output = project::omega(directory, &["--check", "--verbose", "main.omg"]);
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn an_unmarked_clock_sleep_is_refused() {
    let stderr = omega_check(&fixture(
        "source-rust/application/tests/fixtures/effects/fail/clock_sleep_unacknowledged",
    ));
    assert!(
        stderr.contains(&format!("{MISSING_BLOCK} in machine `entry_start`")),
        "{stderr}"
    );
}

#[test]
fn a_marked_clock_sleep_passes_the_acknowledgement_check() {
    let stderr = omega_check(&fixture(
        "source-rust/application/tests/fixtures/effects/pass/clock_sleep_acknowledged",
    ));
    assert!(!stderr.contains(MISSING_BLOCK), "{stderr}");
}
