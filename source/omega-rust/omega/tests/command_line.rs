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

#[test]
fn source_snapshot_is_an_omega_subcommand_and_retains_package_custody() {
    let project = temp_path("source-snapshot");
    std::fs::create_dir(&project).expect("create source snapshot project");
    std::fs::write(
        project.join("build.omg"),
        concat!(
            "machine build(builder: &mut Build) {\n",
            "    builder.application(\"closure-observer\");\n",
            "}\n",
        ),
    )
    .expect("write source snapshot build entry");
    std::fs::write(
        project.join("main.omg"),
        "data Main {}\nmachine Main::main(&mut self) {}\n",
    )
    .expect("write source snapshot source entry");

    let cache = project.with_extension("cache");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(&project)
        .env("OMEGA_SOURCE_CACHE_DIR", &cache)
        .args(["source-snapshot", "--repository-root", ".", "main.omg"])
        .output()
        .expect("run source snapshot");

    assert!(
        output.status.success(),
        "source snapshot failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("package_source_closure"),
        "stdout was: {stdout}"
    );
    assert!(
        stdout.contains("subject_fingerprint"),
        "stdout was: {stdout}"
    );

    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(cache);
}
