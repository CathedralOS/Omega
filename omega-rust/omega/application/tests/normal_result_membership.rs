//! Calls retain the domain facts their normal-result contracts promise.

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn invoke(tier: &str) -> (Output, String) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(tier)
        .join("domains/normal_result_membership");
    let scratch = TemporaryDirectory::create("normal-result-membership", false).unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(fixture.join(name), scratch.path().join(name)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "run",
            "--both",
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
fn ordinary_provider_and_native_results_retain_their_promised_domains() {
    let (output, report) = invoke("run");
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn the_selected_body_must_establish_its_promised_domain() {
    let (output, report) = invoke("fail");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("normal completion does not establish its promised runtime membership"),
        "{report}"
    );
}
