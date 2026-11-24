//! An established byte-view descriptor reaches the declared foreign output boundary.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

#[test]
fn a_foreign_call_receives_the_established_bytes_and_their_extent() {
    let scratch = TemporaryDirectory::create("foreign-established-view", false).unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/views/run/established_foreign_view/build.omg");
    let build = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "build",
            "--compare-tests",
            "--target",
            "linux_x86_64",
            "--build-dir",
        ])
        .arg(scratch.path())
        .arg(fixture)
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(build.status.success(), "{report}");
    for machine in ["writes_to_capture", "returns_normally"] {
        let row = report
            .lines()
            .find(|row| row.contains(&format!("test `{machine}` in group")))
            .unwrap_or_else(|| panic!("{report}"));
        assert!(
            row.contains("passed on Psi interpreter, native")
                && row.contains("native/Psi comparison agrees"),
            "{row}"
        );
    }
    let native = Command::new(scratch.path().join("omega-program"))
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(native.status.success(), "{:?}", native.status);
    assert_eq!(native.stdout, b"okay");
    assert!(native.stderr.is_empty(), "{:?}", native.stderr);
}
