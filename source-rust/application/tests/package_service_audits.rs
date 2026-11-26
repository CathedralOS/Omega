//! Package review retains each reason that an exposed service needs attention.
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::{fs, process::Command};

#[test]
fn root_registration_preserves_each_class_of_an_exposed_core_service() {
    let project = TemporaryDirectory::create("overlapping-service-audits", false).unwrap();
    fs::write(
        project.path().join("build.omg"),
        r#"use {omega}/language/core/process_authority.omg as process;
machine build(builder: @mutable Build) {
    Build::package(builder, "service_audit");
    Build::audit_service::<|process|::StandardOutput>(builder);
    Build::audit_service::<|process|::StandardOutput>(builder);
    Build::audit_service::<|process|::StandardInput>(builder);
}"#,
    )
    .unwrap();
    fs::write(
        project.path().join("main.omg"),
        r#"use {omega}/language/core/process_authority.omg as process;
public machine output_ceiling() reaches |process|::StandardOutput {}"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["packages", "audit", "--target", "linux_x86_64", "--details"])
        .env(
            "OMEGA_LIBRARY_ROOT",
            support::repository_root().join("source/library"),
        )
        .current_dir(project.path())
        .output()
        .unwrap();
    let report = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        output.status.code(),
        Some(3),
        "{report}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary = report.split_once("  dangerous-authority ").unwrap().1;
    let summary = summary.split_once("  external-supplies ").unwrap().0;
    let findings: Vec<_> = summary
        .lines()
        .filter_map(|line| {
            let (class, owned_service) = line.trim().split_once(" toolchain[")?;
            let (_, finding) = owned_service.split_once("]::")?;
            Some((class, finding))
        })
        .collect();
    let service = r#""StandardOutput""#;
    let slack = r#""StandardOutput" declared but absent from checked reach of "service_audit"::"main.omg::output_ceiling""#;
    assert_eq!(
        findings,
        [
            ("Process", service),
            ("ConsumerAudit", service),
            ("Process", slack),
            ("ConsumerAudit", slack)
        ],
        "{summary}"
    );
}

#[test]
fn fresh_audit_reports_each_build_and_a_version_four_lock_refuses_the_chain() {
    let project = TemporaryDirectory::create("audit-staged-package", false).unwrap();
    let root = project.path();
    for (file, source) in [
        (
            "build.omg",
            r#"machine build(b: @mutable Build) { Build::build_entry(b, "prepare.omg"); }"#,
        ),
        (
            "prepare.omg",
            r#"machine build(b: @mutable Build) { Build::build_entry(b, "configure.omg"); }"#,
        ),
        (
            "configure.omg",
            r#"machine build(b: @mutable Build) { Build::package(b, "meter"); }"#,
        ),
        ("main.omg", "public machine value() -> u64 { 17u64 }"),
    ] {
        fs::write(root.join(file), source).unwrap();
    }
    let command = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(root)
            .env(
                "OMEGA_LIBRARY_ROOT",
                support::repository_root().join("source/library"),
            )
            .arg("packages")
            .args(args)
            .args(["--target", "linux_x86_64", "--offline"])
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        (output.status.code(), text)
    };
    let (code, report) = command(&["audit"]);
    assert!(matches!(code, Some(0 | 3)), "{report}");
    assert!(report.contains("fresh-analysis complete"), "{report}");
    assert!(
        report.contains("source-changed unknown (no accepted baseline)"),
        "{report}"
    );
    let stages: Vec<_> = report
        .lines()
        .filter(|line| line.starts_with("preparing Build") || line.starts_with("final Build"))
        .collect();
    assert_eq!(stages.len(), 3, "{report}");
    assert_eq!(
        report.matches("preparation concrete-reach ").count(),
        2,
        "{report}"
    );
    assert!(
        stages[0].starts_with("preparing Build \"build.omg\"")
            && stages[0].ends_with("selected \"prepare.omg\""),
        "{report}"
    );
    assert!(
        stages[1].starts_with("preparing Build \"prepare.omg\"")
            && stages[1].ends_with("selected \"configure.omg\""),
        "{report}"
    );
    assert!(
        stages[2].starts_with("final Build \"configure.omg\""),
        "{report}"
    );
    assert!(!root.join("omega.lock").exists());
    let (code, report) = command(&["update", "--dangerously-accept-all"]);
    assert_ne!(code, Some(0), "{report}");
    assert!(
        report.contains("a version 4 package lock cannot"),
        "{report}"
    );
    assert!(!root.join("omega.lock").exists());
}
