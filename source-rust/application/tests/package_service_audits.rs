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
