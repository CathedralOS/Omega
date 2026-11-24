//! The bundled `core::sha256` package hashes at most 8,192 staged bytes. A
//! longer `msg_n` must be refused with `bad` set and `out32` cleared, never
//! answered with a digest of only the bytes that fit. The fixture also pins
//! two independently known digests: FIPS 180-2's "abc" example and 8,192
//! bytes of `a` from `sha256sum`.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

#[test]
fn sha256_digests_up_to_capacity_and_refuses_longer_messages_in_both_engines() {
    let project = support::repository_root()
        .join("source-rust/application/tests/fixtures/library/run/sha256_digest");
    let scratch = TemporaryDirectory::create("omega-sha256", false).unwrap();
    for name in ["build.omg", "main.omg"] {
        std::fs::copy(project.join(name), scratch.path().join(name)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "build.omg"])
        .current_dir(scratch.path())
        .env(
            "OMEGA_LIBRARY_ROOT",
            support::repository_root().join("source/library"),
        )
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let report = format!("{stdout}{stderr}");
    assert_eq!(output.status.code(), Some(0), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}
