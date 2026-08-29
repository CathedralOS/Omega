use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn omega(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .output()
        .expect("run omega")
}

fn omega_in(directory: &std::path::Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(directory)
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

#[test]
fn package_command_words_do_not_reserve_ordinary_source_filenames() {
    let project = temp_path("install-source");
    std::fs::create_dir(&project).expect("create source project");
    for source_name in ["install.omg", "update.omg"] {
        std::fs::write(project.join(source_name), b"machine main() {}\n")
            .expect("write ordinary source");

        let output = omega_in(&project, &[source_name, "--check"]);

        assert!(
            output.status.success(),
            "ordinary source `{source_name}` should compile: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(project);
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
fn optimizer_rollback_cli_requires_exact_unique_names() {
    let unknown = omega(&[
        "--disable-optimization",
        "control_flow_cleanup",
        "missing.omg",
    ]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&unknown.stderr)
            .contains("unknown exact optimization rollback name `control_flow_cleanup`")
    );

    let duplicate = omega(&[
        "--disable-optimization",
        "ControlFlowCleanup",
        "--disable-optimization",
        "ControlFlowCleanup",
        "missing.omg",
    ]);
    assert_eq!(duplicate.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&duplicate.stderr)
            .contains("optimization rollback repeats `ControlFlowCleanup`")
    );

    let missing = omega(&["--disable-optimization"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&missing.stderr)
            .contains("--disable-optimization requires one exact optimization name")
    );
}

#[test]
fn optimizer_rollback_cli_rejects_check_before_reading_source() {
    let output = omega(&[
        "--check",
        "--disable-optimization",
        "ControlFlowCleanup",
        "missing.omg",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("requires NativeArtifact production"));
    assert!(stderr.contains("`ControlFlowCleanup`"));
    assert!(!stderr.contains("failed to read"));
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
