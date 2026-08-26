use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn omega(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .output()
        .expect("run omega")
}

fn temp_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-package-quarantine-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn assert_quarantined(output: &Output) {
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("quarantined"), "stderr was: {stderr}");
    assert!(stderr.contains("compiler-issued evidence"));
}

#[test]
fn manifest_based_package_commands_are_unavailable() {
    assert_quarantined(&omega(&["audit", "packages"]));
    assert_quarantined(&omega(&["plan", "install"]));
    assert_quarantined(&omega(&["plan", "update"]));
    assert_quarantined(&omega(&["review", "capability-change"]));
}

#[test]
fn prototype_lock_command_cannot_write_omega_lock() {
    let output_path = temp_path("omega-lock");
    let output = omega(&[
        "lock",
        "assemble",
        "--root-package",
        "root",
        "--manifest",
        "attacker.json",
        "--out",
        output_path.to_str().expect("UTF-8 temp path"),
    ]);

    assert_quarantined(&output);
    assert!(!output_path.exists());
}

#[test]
fn production_usage_does_not_advertise_manifest_or_receipt_inputs() {
    let output = omega(&[]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("manifest.json"), "stderr was: {stderr}");
    assert!(!stderr.contains("receipt.json"), "stderr was: {stderr}");
    assert!(!stderr.contains("lock assemble"), "stderr was: {stderr}");
}

#[test]
fn source_audit_requires_an_explicit_supported_adapter() {
    let missing = omega(&["audit", "source", "https://example.invalid/package"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("--kind <local|git>"));

    let unsupported = omega(&[
        "audit",
        "source",
        "--kind",
        "archive",
        "https://example.invalid/package.tar",
    ]);
    assert_eq!(unsupported.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&unsupported.stderr).contains("UnsupportedSourceAdapter"));
}
