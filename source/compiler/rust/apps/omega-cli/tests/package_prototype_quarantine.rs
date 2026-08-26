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

fn assert_quarantined(output: &Output) {
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("quarantined"), "stderr was: {stderr}");
    assert!(stderr.contains("accepted package admission is not implemented"));
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
fn install_and_update_are_quarantined_before_project_mutation() {
    let project = temp_path("install-update");
    std::fs::create_dir(&project).expect("create package command project");
    let build_path = project.join("build.omg");
    let lock_path = project.join("omega.lock");
    std::fs::write(&build_path, b"original build bytes\n").expect("write build sentinel");
    std::fs::write(&lock_path, b"original lock bytes\n").expect("write lock sentinel");

    assert_quarantined(&omega_in(
        &project,
        &["install", "https://example.invalid/package.git"],
    ));
    assert_quarantined(&omega_in(&project, &["update", "dependency"]));

    assert_eq!(
        std::fs::read(&build_path).expect("read build sentinel"),
        b"original build bytes\n"
    );
    assert_eq!(
        std::fs::read(&lock_path).expect("read lock sentinel"),
        b"original lock bytes\n"
    );
    assert!(!project.join("build").exists());
    let _ = std::fs::remove_dir_all(project);
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
